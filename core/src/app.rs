use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::RwLock;

use crate::config::YseConfig;
use crate::crypto::derive_key;
use crate::email::smtp::{SmtpConfig, SmtpSender};
use crate::event::{EventSender, event_channel};
use crate::plugin::process_manager::PluginProcessManager;
use crate::plugin::session::SessionRegistry;
use crate::store::{sqlite::SqliteStorage, Storage};

pub struct CoreState {
    pub store: Arc<dyn Storage>,
    pub config: Arc<RwLock<YseConfig>>,
    pub crypto_key: Arc<RwLock<Option<chacha20poly1305::Key>>>,
    pub sender: Arc<RwLock<Option<SmtpSender>>>,
    pub poller_running: Arc<AtomicBool>,
    pub process_manager: Arc<PluginProcessManager>,
    pub session_registry: Arc<SessionRegistry>,
    pub event_tx: EventSender,
}

impl CoreState {
    pub fn new(db_path: impl AsRef<std::path::Path>, own_address: &str) -> Result<Self, String> {
        let store: Arc<dyn Storage> =
            Arc::new(SqliteStorage::open(db_path).map_err(|e| e.to_string())?);
        let (event_tx, _) = event_channel();
        Ok(Self {
            store,
            config: Arc::new(RwLock::new(YseConfig::default())),
            crypto_key: Arc::new(RwLock::new(None)),
            sender: Arc::new(RwLock::new(None)),
            poller_running: Arc::new(AtomicBool::new(false)),
            process_manager: Arc::new(PluginProcessManager::new()),
            session_registry: Arc::new(SessionRegistry::new(own_address)),
            event_tx,
        })
    }

    pub async fn load_config(&self) {
        if let Ok(Some(json)) = self.store.get_config_value("config").await {
            if let Ok(cfg) = serde_json::from_str::<YseConfig>(&json) {
                let password = cfg.crypto_password.clone();
                let own_name = cfg.own_address.clone();
                let mut w = self.config.write().await;
                *w = cfg;
                drop(w);
                self.session_registry.set_local_name(&own_name);
                if let Ok(hashes) = self.store.get_contact_hashes().await {
                    let map: HashMap<String, String> = hashes.into_iter().collect();
                    self.session_registry.set_contact_hashes(map);
                }
                if !password.is_empty() {
                    if let Ok(key) = derive_key(&password) {
                        *self.crypto_key.write().await = Some(key);
                    }
                }
                {
                    let cfg = self.config.read().await;
                    let smtp_cfg = SmtpConfig {
                        server: cfg.email_smtp_server.clone(),
                        port: cfg.email_smtp_port,
                        username: cfg.email_username.clone(),
                        password: cfg.email_password.clone(),
                    };
                    if !smtp_cfg.server.is_empty() && !smtp_cfg.username.is_empty() {
                        *self.sender.write().await = Some(SmtpSender::new(smtp_cfg));
                    }
                }
            }
        }
    }

    pub async fn update_config(&self, cfg: YseConfig) -> Result<(), String> {
        let smtp_cfg = SmtpConfig {
            server: cfg.email_smtp_server.clone(),
            port: cfg.email_smtp_port,
            username: cfg.email_username.clone(),
            password: cfg.email_password.clone(),
        };
        *self.sender.write().await = Some(SmtpSender::new(smtp_cfg));
        if !cfg.crypto_password.is_empty() {
            let new_key = derive_key(&cfg.crypto_password).map_err(|e| e.to_string())?;
            *self.crypto_key.write().await = Some(new_key);
        }
        self.store
            .set_config_value("config", &serde_json::to_string(&cfg).unwrap())
            .await
            .map_err(|e| e.to_string())?;
        *self.config.write().await = cfg;
        Ok(())
    }

}
