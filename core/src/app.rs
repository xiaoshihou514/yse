use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::identity;

use crate::config::YseConfig;
use crate::crypto::{derive_key, encrypt};
use crate::disguise;
use crate::email::smtp::{SmtpConfig, SmtpSender};
use crate::event::{event_channel, EventSender};
use crate::plugin::process_manager::PluginProcessManager;
use crate::plugin::session::SessionRegistry;
use crate::store::{sqlite::SqliteStorage, Storage};

#[derive(Clone)]
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

    pub fn set_app_data_dir(&self, dir: impl AsRef<std::path::Path>) {
        self.process_manager
            .set_app_data_dir(dir.as_ref().display().to_string());
    }

    pub async fn load_config(&self) {
        let Some(json) = self.store.get_config_value("config").await.ok().flatten() else {
            return;
        };
        let Ok(cfg) = serde_json::from_str::<YseConfig>(&json) else {
            return;
        };

        let password = cfg.crypto_password.clone();
        let mut w = self.config.write().await;
        *w = cfg;
        w.own_address = "me".into();
        drop(w);
        self.session_registry.set_local_name("me");

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
        let json = serde_json::to_string(&cfg).map_err(|e| e.to_string())?;
        self.store
            .set_config_value("config", &json)
            .await
            .map_err(|e| e.to_string())?;
        *self.config.write().await = cfg;
        Ok(())
    }

    /// Serialize + encrypt + send a message via SMTP.
    /// `attachments`: (enc_name, encrypted_bytes) pairs for file attachments.
    pub async fn send_encrypted(
        &self,
        msg: &crate::message::Message,
        attachments: Vec<(String, Vec<u8>)>,
    ) -> Result<(), String> {
        let email_user = self.config.read().await.email_username.clone();
        let key_guard = self.crypto_key.read().await;
        let key = key_guard.as_ref().ok_or("crypto key not set")?;
        let payload = msg.to_json().map_err(|e| e.to_string())?;
        let encrypted = encrypt(key, &payload).map_err(|e| e.to_string())?;
        let mut email_body = "[YSE 加密消息]\n\n".as_bytes().to_vec();
        email_body.extend(&encrypted);
        let sender_name = identity::parse_address(&msg.from_addr)
            .map(|(n, _, _)| n.to_string())
            .unwrap_or_else(|| String::from("未知"));
        let email_subject = format!("[盐水鹅] 来自 {} 的消息", sender_name);
        if let Some(sender) = self.sender.read().await.as_ref() {
            let d = disguise::disguise();
            sender
                .send(
                    (&email_user, &d.display_name),
                    &email_user,
                    &email_subject,
                    email_body,
                    attachments,
                )
                .await
                .map_err(|e| format!("SMTP send failed: {}", e))
        } else {
            Err("SMTP not configured".into())
        }
    }
}
