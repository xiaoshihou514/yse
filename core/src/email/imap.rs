use imap::extensions::idle::{self, WaitOutcome};
use log::{debug, info, warn};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use thiserror::Error;
use tokio::time::{interval, Duration};

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

pub struct ImapPoller {
    config: ImapConfig,
    running: Arc<AtomicBool>,
    last_uid: Arc<Mutex<Option<u32>>>,
}

impl ImapPoller {
    pub fn new(config: ImapConfig, last_uid: Option<u32>) -> Self {
        Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
            last_uid: Arc::new(Mutex::new(last_uid)),
        }
    }

    pub fn running_flag(&self) -> Arc<AtomicBool> {
        self.running.clone()
    }

    pub fn last_uid_arc(&self) -> Arc<Mutex<Option<u32>>> {
        self.last_uid.clone()
    }

    /// Replace the internal running flag with an externally-owned one,
    /// enabling the caller (e.g. YseState.poller_running) to stop the poller
    /// by setting it to `false` from another context.
    pub fn set_running_flag(&mut self, flag: Arc<AtomicBool>) {
        self.running = flag;
    }

    pub fn connect_sync(&self) -> Result<imap::Session<imap::Connection>, ImapError> {
        let noop: LogFn = Arc::new(|_, _| {});
        self.connect_sync_log(&noop)
    }

    fn connect_sync_log(
        &self,
        log_fn: &LogFn,
    ) -> Result<imap::Session<imap::Connection>, ImapError> {
        log_fn("debug", "IMAP: connecting...".into());

        let client = imap::ClientBuilder::new(&self.config.server, self.config.port)
            .connect()
            .map_err(|e| ImapError::Connect(e.to_string()))?;

        log_fn(
            "debug",
            format!("IMAP: connected to {}", self.config.server),
        );

        let mut session = client
            .login(&self.config.username, &self.config.password)
            .map_err(|(e, _)| ImapError::Login(e.to_string()))?;

        log_fn("debug", "IMAP: login OK".into());
        debug!("IMAP: login OK");

        // Some servers (QQ Mail, 163/Coremail) require the ID command (RFC 2971)
        // before SELECT/EXAMINE. imap-proto >=0.16 natively supports parsing the
        // `* ID (...)` response via `Response::Id`.
        match session.run_command_and_check_ok(r#"ID ("name" "yse" "version" "1.0")"#) {
            Ok(()) => log_fn("debug", "IMAP: ID exchange OK".into()),
            Err(e) => {
                let msg = format!("IMAP: ID exchange failed: {e}");
                warn!("{msg}");
                log_fn("warn", msg);
            }
        }

        log_fn("debug", "IMAP: selecting INBOX...".into());

        // Try SELECT first, fall back to EXAMINE.
        if let Err(e) = session.select("INBOX").or_else(|e| {
            let msg = format!("IMAP: SELECT INBOX failed: {e}");
            info!("{msg}");
            log_fn("warn", msg);
            session.examine("INBOX")
        }) {
            let msg = format!("IMAP: EXAMINE INBOX also failed: {e}");
            warn!("{msg}");
            log_fn("error", msg);
            return Err(ImapError::Connect(format!(
                "select/examine INBOX failed: {e}"
            )));
        }

        log_fn("debug", "IMAP: SELECT/EXAMINE INBOX OK".into());
        debug!("IMAP: SELECT/EXAMINE INBOX OK");
        Ok(session)
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
        debug!("IMAP: initial connect");
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
            debug!("IMAP: starting poll cycle");
            match self.connect_sync_log(&log_fn) {
                Ok(mut session) => {
                    debug!("IMAP: connected, fetching new messages");
                    self.fetch_new_sync(&mut session, &mut on_message, &log_fn);
                }
                Err(e) => {
                    let msg = format!("IMAP reconnect failed: {:?}", e);
                    warn!("{}", msg);
                    log_fn("error", msg);
                }
            }
        }
        debug!("IMAP: poller stopped");
    }

    /// Run the IDLE loop with a log callback.
    /// Uses a dedicated OS thread (Session is not Send).
    /// Falls back to 10s polling if the server does not support IDLE.
    pub async fn run_idle_with_log<F>(self, mut on_message: F, log_fn: LogFn)
    where
        F: FnMut(Vec<u8>) + Send + 'static,
    {
        debug!("IMAP IDLE: starting");
        thread::spawn(move || {
            let mut backoff: u64 = 1;

            'outer: while self.running.load(Ordering::SeqCst) {
                let mut session = match self.connect_sync_log(&log_fn) {
                    Ok(s) => s,
                    Err(e) => {
                        let msg = format!("IMAP IDLE connect failed: {}", e);
                        warn!("{}", msg);
                        log_fn("error", msg);
                        thread::sleep(Duration::from_secs(backoff));
                        backoff = (backoff * 2).min(60);
                        continue;
                    }
                };
                backoff = 1;

                log_fn("debug", "IMAP IDLE: initial fetch".into());
                self.fetch_new_sync(&mut session, &mut on_message, &log_fn);

                let has_idle = session
                    .capabilities()
                    .ok()
                    .map(|caps| caps.has_str("IDLE"))
                    .unwrap_or(false);

                if !has_idle {
                    warn!("IMAP IDLE not supported, falling back to 10s polling");
                    log_fn(
                        "warn",
                        "IMAP IDLE not supported, falling back to 10s polling".into(),
                    );
                    while self.running.load(Ordering::SeqCst) {
                        if session.noop().is_err() {
                            break;
                        }
                        self.fetch_new_sync(&mut session, &mut on_message, &log_fn);
                        for _ in 0..60 {
                            thread::sleep(Duration::from_secs(1));
                            if !self.running.load(Ordering::SeqCst) {
                                break 'outer;
                            }
                        }
                    }
                    continue 'outer;
                }

                log_fn("info", "IMAP IDLE: entering IDLE loop".into());
                'idle: loop {
                    if !self.running.load(Ordering::SeqCst) {
                        break 'outer;
                    }

                    let result = {
                        let mut handle = session.idle();
                        handle.wait_while(idle::stop_on_any)
                    };

                    match result {
                        Ok(WaitOutcome::MailboxChanged) => {
                            log_fn("debug", "IMAP IDLE: mailbox changed".into());
                            self.fetch_new_sync(&mut session, &mut on_message, &log_fn);
                        }
                        Ok(WaitOutcome::TimedOut) => {
                            log_fn("debug", "IMAP IDLE: timeout, refreshing".into());
                            self.fetch_new_sync(&mut session, &mut on_message, &log_fn);
                        }
                        Err(e) => {
                            let msg = format!("IMAP IDLE error: {}, reconnecting", e);
                            warn!("{}", msg);
                            log_fn("warn", msg);
                            break 'idle;
                        }
                    }
                }
            }
            debug!("IMAP IDLE: stopped");
        });
    }

    fn fetch_new_sync<F>(
        &self,
        session: &mut imap::Session<imap::Connection>,
        on_message: &mut F,
        log_fn: &LogFn,
    ) where
        F: FnMut(Vec<u8>),
    {
        let last_uid = { *self.last_uid.lock().unwrap() };
        debug!("IMAP: last_uid = {:?}", last_uid);

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
        debug!("IMAP: UID SEARCH ALL returned {} UIDs", all_uids.len());

        // Filter to only UIDs > last_uid (new messages)
        let new_uids: Vec<u32> = all_uids
            .iter()
            .copied()
            .filter(|u| last_uid.map(|l| *u > l).unwrap_or(true))
            .collect();

        if new_uids.is_empty() {
            let msg = format!("IMAP: no new messages ({} total)", all_uids.len());
            debug!("{}", msg);
            log_fn("debug", msg);
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
        debug!("IMAP: fetching {} new UID(s): {}", new_uids.len(), uid_set);
        log_fn(
            "debug",
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

        debug!("IMAP: UID FETCH returned {} message(s)", fetches.len());
        log_fn(
            "debug",
            format!("IMAP: got {} message body / bodies", fetches.len()),
        );

        for msg in fetches.iter() {
            if let Some(body) = msg.body() {
                on_message(body.to_vec());
            }
        }
    }
}
