use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use thiserror::Error;
use tokio::time::{interval, Duration};
use tracing::{info, warn};

type LogFn = Arc<dyn Fn(&str, String) + Send + Sync>;

#[derive(Error, Debug)]
pub enum ImapError {
    #[error("connection failed: {0}")]
    Connect(String),
    #[error("login failed: {0}")]
    Login(String),
    #[error("fetch failed: {0}")]
    Fetch(String),
    #[error("TLS error: {0}")]
    Tls(String),
}

#[derive(Debug, Clone)]
pub struct ImapConfig {
    pub server: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

type Stream = native_tls::TlsStream<std::net::TcpStream>;

pub struct ImapPoller {
    config: ImapConfig,
    running: Arc<AtomicBool>,
    last_uid: Arc<Mutex<Option<u32>>>,
}

impl ImapPoller {
    pub fn new(config: ImapConfig) -> Self {
        Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
            last_uid: Arc::new(Mutex::new(None)),
        }
    }

    pub fn running_flag(&self) -> Arc<AtomicBool> {
        self.running.clone()
    }

    /// Replace the internal running flag with an externally-owned one,
    /// enabling the caller (e.g. YseState.poller_running) to stop the poller
    /// by setting it to `false` from another context.
    pub fn set_running_flag(&mut self, flag: Arc<AtomicBool>) {
        self.running = flag;
    }

    pub fn connect_sync(&self) -> Result<imap::Session<Stream>, ImapError> {
        let noop: LogFn = Arc::new(|_, _| {});
        self.connect_sync_log(&noop)
    }

    fn connect_sync_log(&self, log_fn: &LogFn) -> Result<imap::Session<Stream>, ImapError> {
        let tls = native_tls::TlsConnector::builder()
            .build()
            .map_err(|e| ImapError::Tls(e.to_string()))?;

        log_fn("info", "IMAP: connecting...".into());

        let client = imap::connect(
            (self.config.server.clone(), self.config.port),
            self.config.server.as_str(),
            &tls,
        )
        .map_err(|e| ImapError::Connect(e.to_string()))?;

        log_fn("info", format!("IMAP: connected to {}", self.config.server));

        let mut session = client
            .login(&self.config.username, &self.config.password)
            .map_err(|(e, _)| ImapError::Login(e.to_string()))?;

        log_fn("info", "IMAP: login OK".into());
        info!("IMAP: login OK");

        // Some servers (e.g. 163 / Coremail) require an ID command before SELECT.
        // We bypass the imap crate's parser (it can't handle `* ID (...)`) and
        // send the command + read the response manually through the raw BufStream.
        match unsafe { Self::send_id_raw(&mut session) } {
            Ok(_) => {
                log_fn("info", "IMAP: ID exchange OK".into());
                info!("IMAP: ID exchange OK");
            }
            Err(e) => {
                let msg = format!("IMAP: ID exchange skipped/failed: {e}");
                info!("{msg}");
                log_fn("info", msg);
            }
        }

        log_fn("info", "IMAP: selecting INBOX...".into());

        // Try SELECT first, fall back to EXAMINE.
        if session
            .select("INBOX")
            .or_else(|_| session.examine("INBOX"))
            .is_err()
        {
            return Err(ImapError::Connect("select/examine INBOX failed".into()));
        }

        log_fn("info", "IMAP: SELECT/EXAMINE INBOX OK".into());
        info!("IMAP: SELECT/EXAMINE INBOX OK");
        Ok(session)
    }

    /// Send the IMAP `ID` command and consume both response lines manually,
    /// bypassing `imap_proto` which cannot parse `* ID (...)` responses.
    ///
    /// After `login` the internal tag is 1. We send `a0002 ID …` then consume
    /// `* ID (…)` + `a0002 OK ID completed`. The next crate command (`SELECT`)
    /// increments to 2 and sends `a0002 SELECT …` — tags are opaque to the
    /// server, so re‑using `a0002` is fine.
    ///
    /// # Safety
    /// Transmutes `Session` → `BufStream` to read/write the raw IMAP protocol.
    unsafe fn send_id_raw(session: &mut imap::Session<Stream>) -> std::io::Result<()> {
        use std::io::{BufRead, Write};
        let bufstream = session as *mut imap::Session<Stream> as *mut bufstream::BufStream<Stream>;
        let bufstream = &mut *bufstream;

        bufstream.write_all(b"a0002 ID (\"name\" \"yse\" \"version\" \"1.0\")\r\n")?;
        bufstream.flush()?;

        let mut raw = String::new();
        bufstream.read_line(&mut raw)?;
        info!("IMAP: ID untagged: {}", raw.trim());

        raw.clear();
        bufstream.read_line(&mut raw)?;
        info!("IMAP: ID tagged: {}", raw.trim());

        Ok(())
    }

    pub async fn run<F>(self, on_message: F)
    where
        F: FnMut(Vec<u8>) + Send + 'static,
    {
        self.run_with_log(on_message, Arc::new(|_, _| {})).await;
    }

    /// Run the poller with a log callback. The callback receives (level, message).
    /// The caller must ensure `self.running` is set to `true` before calling this
    /// (e.g. via `set_running_flag` + `store(true, …)`). Otherwise the while loop
    /// will not execute.
    pub async fn run_with_log<F>(self, mut on_message: F, log_fn: LogFn)
    where
        F: FnMut(Vec<u8>) + Send + 'static,
    {
        match self.connect_sync_log(&log_fn) {
            Ok(mut session) => {
                self.fetch_new_sync(&mut session, &mut on_message, &log_fn);
            }
            Err(e) => {
                let msg = format!("IMAP initial connect failed: {}", e);
                warn!("{}", msg);
                log_fn("error", msg);
            }
        }

        let mut tick = interval(Duration::from_secs(10));
        while self.running.load(Ordering::SeqCst) {
            tick.tick().await;
            match self.connect_sync_log(&log_fn) {
                Ok(mut session) => {
                    self.fetch_new_sync(&mut session, &mut on_message, &log_fn);
                }
                Err(e) => {
                    let msg = format!("IMAP reconnect failed: {:?}", e);
                    warn!("{}", msg);
                    log_fn("error", msg);
                }
            }
        }
    }

    fn fetch_new_sync<F>(
        &self,
        session: &mut imap::Session<Stream>,
        on_message: &mut F,
        log_fn: &LogFn,
    ) where
        F: FnMut(Vec<u8>),
    {
        let last_uid = { *self.last_uid.lock().unwrap() };
        info!("IMAP: last_uid = {:?}", last_uid);

        // Use "ALL" for compatibility (QQ Mail rejects "UID SEARCH UID N:*").
        let all_uids = match session.uid_search("ALL") {
            Ok(ids) => ids,
            Err(e) => {
                let msg = format!("IMAP UID SEARCH ALL failed: {}", e);
                warn!("{}", msg);
                log_fn("error", msg);
                return;
            }
        };
        info!("IMAP: UID SEARCH ALL returned {} UIDs", all_uids.len());

        // Filter to only UIDs > last_uid (new messages)
        let new_uids: Vec<u32> = all_uids
            .iter()
            .copied()
            .filter(|u| last_uid.map(|l| *u > l).unwrap_or(true))
            .collect();

        if new_uids.is_empty() {
            let msg = format!("IMAP: no new messages ({} total)", all_uids.len());
            info!("{}", msg);
            log_fn("info", msg);
            return;
        }

        // Track the highest UID ever seen
        if let Some(&max_uid) = all_uids.iter().max() {
            let mut last = self.last_uid.lock().unwrap();
            if last.map(|l| max_uid > l).unwrap_or(true) {
                *last = Some(max_uid);
            }
        }

        let uid_list: Vec<String> = new_uids.iter().map(|u| u.to_string()).collect();
        let uid_set = uid_list.join(",");
        info!("IMAP: fetching {} new UID(s): {}", new_uids.len(), uid_set);
        log_fn(
            "info",
            format!("IMAP: fetching {} new message(s)...", new_uids.len()),
        );

        let fetches = match session.uid_fetch(uid_set.as_str(), "(BODY[])") {
            Ok(f) => f,
            Err(e) => {
                let msg = format!("IMAP UID FETCH failed: {}", e);
                warn!("{}", msg);
                log_fn("error", msg);
                return;
            }
        };

        info!("IMAP: UID FETCH returned {} message(s)", fetches.len());
        log_fn(
            "info",
            format!("IMAP: got {} message body / bodies", fetches.len()),
        );

        for msg in fetches.iter() {
            if let Some(body) = msg.body() {
                on_message(body.to_vec());
            }
        }
    }
}
