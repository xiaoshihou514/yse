use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tokio::time::{interval, Duration};
use thiserror::Error;
use tracing::warn;

type WarnFn = Arc<dyn Fn(String) + Send + Sync>;

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
        let tls = native_tls::TlsConnector::builder()
            .build()
            .map_err(|e| ImapError::Tls(e.to_string()))?;

        let client = imap::connect(
            (self.config.server.clone(), self.config.port),
            self.config.server.as_str(),
            &tls,
        )
        .map_err(|e| ImapError::Connect(e.to_string()))?;

        let session = client
            .login(&self.config.username, &self.config.password)
            .map_err(|(e, _)| ImapError::Login(e.to_string()))?;

        Ok(session)
    }

    pub async fn run<F>(self, on_message: F)
    where
        F: FnMut(Vec<u8>) + Send + 'static,
    {
        self.run_with_warn(on_message, Arc::new(|_| {})).await;
    }

    pub async fn run_with_warn<F>(self, mut on_message: F, on_warn: WarnFn)
    where
        F: FnMut(Vec<u8>) + Send + 'static,
    {
        self.running.store(true, Ordering::SeqCst);

        match self.connect_sync() {
            Ok(mut session) => {
                self.fetch_new_sync(&mut session, &mut on_message, &on_warn);
            }
            Err(e) => {
                let msg = format!("IMAP initial connect failed: {}", e);
                warn!("{}", msg);
                on_warn(msg);
            }
        }

        let mut tick = interval(Duration::from_secs(10));
        while self.running.load(Ordering::SeqCst) {
            tick.tick().await;
            match self.connect_sync() {
                Ok(mut session) => {
                    self.fetch_new_sync(&mut session, &mut on_message, &on_warn);
                }
                Err(e) => {
                    let msg = format!("IMAP reconnect failed: {:?}", e);
                    warn!("{}", msg);
                    on_warn(msg);
                }
            }
        }
    }

    fn fetch_new_sync<F>(&self, session: &mut imap::Session<Stream>, on_message: &mut F, on_warn: &WarnFn)
    where
        F: FnMut(Vec<u8>),
    {
        let last_uid = { *self.last_uid.lock().unwrap() };

        let search_query = if let Some(uid) = last_uid {
            format!("UID {}:*", uid + 1)
        } else {
            "ALL".to_string()
        };

        let uids = match session.uid_search(&search_query) {
            Ok(ids) => ids,
            Err(e) => {
                let msg = format!("IMAP search failed: {}", e);
                warn!("{}", msg);
                on_warn(msg);
                return;
            }
        };

        if uids.is_empty() {
            on_warn("IMAP fetch: no new messages".into());
            return;
        }

        let uid_list: Vec<String> = uids.iter().map(|u| u.to_string()).collect();
        let uid_set = uid_list.join(",");

        let fetches = match session.uid_fetch(uid_set.as_str(), "(BODY[])") {
            Ok(f) => f,
            Err(e) => {
                let msg = format!("IMAP fetch failed: {}", e);
                warn!("{}", msg);
                on_warn(msg);
                return;
            }
        };

        for msg in fetches.iter() {
            if let Some(body) = msg.body() {
                on_message(body.to_vec());
            }
            if let Some(uid) = msg.uid {
                let mut last = self.last_uid.lock().unwrap();
                if last.map_or(true, |l| uid > l) {
                    *last = Some(uid);
                }
            }
        }
    }
}
