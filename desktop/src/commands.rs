use std::sync::Arc;
use tokio::sync::RwLock;
use tauri::{Emitter, State};
use yse_core::{
    config::YseConfig,
    crypto::{derive_key, decrypt, encrypt},
    disguise,
    email::{
        imap::{ImapConfig, ImapPoller},
        parser::parse_incoming,
        smtp::{SmtpConfig, SmtpSender},
    },
    event::{CoreEvent, EventSender},
    message::Message,
    plugin::process::PluginManager,
    router::Router,
    store::{PluginConfig, Storage, sqlite::SqliteStorage},
};

use serde::Serialize;

// ---------------------------------------------------------------------------
// Frontend-facing types
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
pub struct LogEntry {
    pub level: String,
    pub message: String,
    pub timestamp: u64,
}

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

pub struct YseState {
    pub store: Arc<dyn Storage>,
    pub config: RwLock<YseConfig>,
    pub crypto_key: RwLock<Option<chacha20poly1305::Key>>,
    pub sender: RwLock<Option<SmtpSender>>,
    pub poller_running: Arc<std::sync::atomic::AtomicBool>,
    pub plugin_manager: Arc<PluginManager>,
    #[allow(dead_code)]
    pub router: Arc<Router>,
    pub log_buffer: Arc<RwLock<Vec<LogEntry>>>,
    pub event_tx: EventSender,
}

impl YseState {
    pub fn new(db_path: std::path::PathBuf) -> Result<Self, String> {
        let store: Arc<dyn Storage> =
            Arc::new(SqliteStorage::open(db_path).map_err(|e| e.to_string())?);
        let (event_tx, _) = yse_core::event::event_channel();
        let plugin_manager = Arc::new(PluginManager::new());
        let router = Arc::new(Router::new(plugin_manager.clone()));

        Ok(Self {
            store,
            config: RwLock::new(YseConfig::default()),
            crypto_key: RwLock::new(None),
            sender: RwLock::new(None),
            poller_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            plugin_manager,
            router,
            log_buffer: Arc::new(RwLock::new(Vec::new())),
            event_tx,
        })
    }

    /// Load persisted config from DB, called from Tauri setup (tokio runtime available).
    pub async fn load_config(&self) {
        if let Ok(Some(json)) = self.store.get_config_value("config").await {
            if let Ok(cfg) = serde_json::from_str::<YseConfig>(&json) {
                let mut w = self.config.write().await;
                *w = cfg;
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

        self.store
            .set_config_value("config", &serde_json::to_string(&cfg).unwrap())
            .await
            .map_err(|e| e.to_string())?;

        *self.config.write().await = cfg;
        Ok(())
    }

    pub fn log(&self, level: &str, message: String) {
        let entry = LogEntry {
            level: level.into(),
            message,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };
        let _ = self.event_tx.send(CoreEvent::Log {
            level: entry.level.clone(),
            message: entry.message.clone(),
        });

        let buf = self.log_buffer.clone();
        tokio::spawn(async move {
            let mut b = buf.write().await;
            b.push(entry);
            if b.len() > 1000 {
                let keep = b.len() - 500;
                b.drain(0..keep);
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Tauri Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn send_message(
    state: State<'_, YseState>,
    to: String,
    text: String,
    _files: Option<Vec<String>>,
) -> Result<(), String> {
    let cfg = state.config.read().await;
    let key = state.crypto_key.read().await;
    let key = key.as_ref().ok_or("crypto key not set")?;
    let sender = state.sender.read().await;
    let sender = sender.as_ref().ok_or("SMTP not configured")?;

    let email_username = cfg.email_username.clone();
    let msg = Message::new(cfg.own_address.clone(), to.clone(), Some(text));
    drop(cfg);

    let payload = msg.to_json().map_err(|e| e.to_string())?;
    let encrypted = encrypt(key, &payload).map_err(|e| e.to_string())?;
    let d = disguise::disguise();

    sender
        .send(
            (&d.from_addr, &d.display_name),
            &email_username,
            encrypted,
            vec![],
        )
        .await
        .map_err(|e| e.to_string())?;

    state.store.save_message(&msg).await.map_err(|e| e.to_string())?;

    // Also dispatch locally if to address matches a plugin mapping
    {
        let cfg = state.config.read().await;
        let mappings: Vec<(String, String)> = cfg
            .plugin_mappings
            .iter()
            .map(|m| (m.virtual_addr.clone(), m.plugin_id.clone()))
            .collect();
        state
            .plugin_manager
            .dispatch_message(
                &msg.to_addr,
                &msg.from_addr,
                msg.text.as_deref(),
                msg.meta.as_ref(),
                msg.files.as_ref(),
                &mappings,
            )
            .await;
    }

    state.log("info", format!("sent message to {}", to));
    Ok(())
}

#[tauri::command]
pub async fn get_messages(
    state: State<'_, YseState>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<Message>, String> {
    state
        .store
        .list_messages(limit.unwrap_or(50), offset.unwrap_or(0))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_config(state: State<'_, YseState>) -> Result<YseConfig, String> {
    Ok(state.config.read().await.clone())
}

#[tauri::command]
pub async fn save_config(state: State<'_, YseState>, config: YseConfig) -> Result<(), String> {
    state.update_config(config).await
}

#[tauri::command]
pub async fn set_crypto_password(
    state: State<'_, YseState>,
    password: String,
) -> Result<(), String> {
    let key = derive_key(&password).map_err(|e| e.to_string())?;
    *state.crypto_key.write().await = Some(key);
    state.log("info", "crypto key derived".into());
    Ok(())
}

#[tauri::command]
pub async fn start_polling(
    state: State<'_, YseState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    // Read config and key inside a block so locks are dropped before spawn
    let (imap_cfg, key, store, own_addr, mappings, plugin_manager) = {
        let cfg = state.config.read().await;
        let imap = ImapConfig {
            server: cfg.email_imap_server.clone(),
            port: cfg.email_imap_port,
            username: cfg.email_username.clone(),
            password: cfg.email_password.clone(),
        };
        let mappings: Vec<(String, String)> = cfg
            .plugin_mappings
            .iter()
            .map(|m| (m.virtual_addr.clone(), m.plugin_id.clone()))
            .collect();
        drop(cfg);

        let key = state
            .crypto_key
            .read()
            .await
            .clone()
            .ok_or("crypto key not set")?;

        (imap, key, state.store.clone(), state.config.read().await.own_address.clone(), mappings, state.plugin_manager.clone())
    };

    state.log("info", "IMAP polling started".into());

    let poller = ImapPoller::new(imap_cfg);

    tokio::spawn(async move {
        poller
            .run(move |raw_email| {
                let parsed = match parse_incoming(&raw_email) {
                    Ok(p) => p,
                    Err(_) => return,
                };
                let decrypted = match decrypt(&key, &parsed.encrypted_body) {
                    Ok(d) => d,
                    Err(_) => return,
                };
                let msg = match Message::from_json(&decrypted) {
                    Ok(m) => m,
                    Err(_) => return,
                };

                let store = store.clone();
                let handle = app_handle.clone();
                let own = own_addr.clone();
                let pm = plugin_manager.clone();
                let mapping = mappings.clone();

                tokio::spawn(async move {
                    let _ = store.save_message(&msg).await;

                    // Dispatch to plugins if address matches
                    pm.dispatch_message(
                        &msg.to_addr,
                        &msg.from_addr,
                        msg.text.as_deref(),
                        msg.meta.as_ref(),
                        msg.files.as_ref(),
                        &mapping,
                    )
                    .await;

                    if msg.to_addr == own || msg.from_addr == own {
                        let _ = handle.emit("new-message", &msg);
                    }
                });
            })
            .await;
    });

    Ok(())
}

#[tauri::command]
pub async fn stop_polling(state: State<'_, YseState>) -> Result<(), String> {
    state
        .poller_running
        .store(false, std::sync::atomic::Ordering::SeqCst);
    state.log("info", "IMAP polling stopped".into());
    Ok(())
}

#[tauri::command]
pub async fn list_plugins(
    state: State<'_, YseState>,
) -> Result<Vec<PluginConfig>, String> {
    state.store.list_plugins().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_plugin(
    state: State<'_, YseState>,
    id: String,
    name: String,
    exec_path: String,
) -> Result<(), String> {
    let pc = PluginConfig {
        id: id.clone(),
        name,
        exec_path: exec_path.clone(),
        args: vec![],
        enabled: true,
    };
    state.store.save_plugin(&pc).await.map_err(|e| e.to_string())?;
    state
        .plugin_manager
        .start_plugin(&id, &exec_path, &[])
        .await?;
    state.log("info", format!("plugin added: {}", id));
    Ok(())
}

#[tauri::command]
pub async fn remove_plugin(state: State<'_, YseState>, id: String) -> Result<(), String> {
    let _ = state.plugin_manager.stop_plugin(&id).await;
    state.store.delete_plugin(&id).await.map_err(|e| e.to_string())?;
    state.log("info", format!("plugin removed: {}", id));
    Ok(())
}

#[tauri::command]
pub async fn toggle_plugin(
    state: State<'_, YseState>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    if enabled {
        let plugins = state.store.list_plugins().await.map_err(|e| e.to_string())?;
        let p = plugins
            .iter()
            .find(|p| p.id == id)
            .ok_or("plugin not found")?;
        state
            .plugin_manager
            .start_plugin(&id, &p.exec_path, &p.args)
            .await?;
    } else {
        state.plugin_manager.stop_plugin(&id).await?;
    }
    state
        .store
        .save_plugin(&PluginConfig {
            id,
            name: String::new(),
            exec_path: String::new(),
            args: vec![],
            enabled,
        })
        .await
        .map_err(|e| e.to_string())?;
    state.log("info", "plugin toggled".into());
    Ok(())
}

#[tauri::command]
pub async fn start_plugin(
    state: State<'_, YseState>,
    id: String,
) -> Result<(), String> {
    let plugins = state.store.list_plugins().await.map_err(|e| e.to_string())?;
    let p = plugins.iter().find(|p| p.id == id).ok_or("plugin not found")?;
    state
        .plugin_manager
        .start_plugin(&id, &p.exec_path, &p.args)
        .await?;
    state.log("info", format!("plugin started: {}", id));
    Ok(())
}

#[tauri::command]
pub async fn stop_plugin(state: State<'_, YseState>, id: String) -> Result<(), String> {
    match state.plugin_manager.stop_plugin(&id).await {
        Ok(_) => state.log("info", format!("plugin stopped: {}", id)),
        Err(_) => state.log("info", format!("plugin {} not running, skipping", id)),
    }
    Ok(())
}

#[tauri::command]
pub async fn list_running_plugins(state: State<'_, YseState>) -> Result<Vec<String>, String> {
    Ok(state.plugin_manager.list_running().await)
}

#[tauri::command]
pub async fn get_logs(
    state: State<'_, YseState>,
    limit: Option<usize>,
) -> Result<Vec<LogEntry>, String> {
    let buf = state.log_buffer.read().await;
    let limit = limit.unwrap_or(100);
    let start = if buf.len() > limit { buf.len() - limit } else { 0 };
    Ok(buf[start..].to_vec())
}

#[tauri::command]
pub async fn test_email(
    _state: State<'_, YseState>,
    server: String,
    port: u16,
    username: String,
    password: String,
) -> Result<String, String> {
    let p = ImapPoller::new(ImapConfig {
        server,
        port,
        username,
        password,
    });
    let _session = p.connect_sync().map_err(|e| format!("IMAP 连接失败: {}", e))?;
    Ok("邮箱连接正常".into())
}
