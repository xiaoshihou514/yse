use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use thiserror::Error;
use tokio::time::{interval, Duration};
use tracing::{info, warn};

use std::io::BufRead;

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
        match session.run_command_and_check_ok(r#"ID ("name" "yse" "version" "1.0")"#) {
            Ok(_) => {
                log_fn("info", "IMAP: ID command OK".into());
                info!("IMAP: ID command OK");
            }
            Err(ref e) if matches!(e, imap::error::Error::Parse(_)) => {
                log_fn(
                    "info",
                    "IMAP: ID response unparseable (Coremail style), draining buffer".into(),
                );
                info!("IMAP: ID response unparseable (Coremail style), draining buffer");
                // Set a short read timeout so drain doesn't hang if the
                // tagged response hasn't arrived in the BufReader yet.
                let drain_ok = unsafe { Self::drain_response_line(&mut session) }.is_ok();
                if drain_ok {
                    log_fn("info", "IMAP: drained ID response line".into());
                } else {
                    let msg = "IMAP: drain ID response line failed/timed out, continuing anyway";
                    warn!("{}", msg);
                    log_fn("warn", msg.into());
                }
            }
            Err(e) => {
                let msg = format!("IMAP: ID command not supported: {e}");
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

    /// After sending an `ID` command, `imap_proto` cannot parse `* ID (...)` responses.
    /// The untagged line is consumed but the tagged response remains in the BufReader's
    /// internal buffer. We drain it here to avoid tag mismatch panic on the next command.
    ///
    /// # Safety
    /// Transmutes `Session` → `BufStream` to access the internal BufReader.
    /// Layout: `Session { conn: Connection { stream: BufStream, … }, … }`.
    /// Both types share the same starting address (Rust preserves field order).
    unsafe fn drain_response_line(session: &mut imap::Session<Stream>) -> std::io::Result<()> {
        let bufstream = session as *mut imap::Session<Stream> as *mut bufstream::BufStream<Stream>;
        let bufstream = &mut *bufstream;

        // Set a 3 s read timeout on the TCP socket so we don't hang
        // indefinitely if the tagged response hasn't been buffered yet.
        {
            let raw: &mut Stream = bufstream.get_mut();
            let _ = raw
                .get_ref()
                .set_read_timeout(Some(std::time::Duration::from_secs(3)));
        }

        let mut line = String::new();
        let n = bufstream.read_line(&mut line)?;

        // Restore infinite timeout.
        {
            let raw: &mut Stream = bufstream.get_mut();
            let _ = raw.get_ref().set_read_timeout(None);
        }

        if n > 0 {
            info!("IMAP: drained response line: {}", line.trim());
        }
        Ok(())
    }

    pub async fn run<F>(self, on_message: F)
    where
        F: FnMut(Vec<u8>) + Send + 'static,
    {
        self.run_with_log(on_message, Arc::new(|_, _| {})).await;
    }

    /// Run the poller with a log callback. The callback receives (level, message).
    pub async fn run_with_log<F>(self, mut on_message: F, log_fn: LogFn)
    where
        F: FnMut(Vec<u8>) + Send + 'static,
    {
        self.running.store(true, Ordering::SeqCst);

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
