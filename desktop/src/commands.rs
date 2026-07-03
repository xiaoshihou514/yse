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

/// Emit a log entry: push to Tauri event (if app_handle ready) + buffer.
fn log_emit(
    app_handle: &Arc<std::sync::Mutex<Option<tauri::AppHandle>>>,
    log_buffer: &Arc<std::sync::Mutex<Vec<LogEntry>>>,
    level: &str,
    message: String,
) {
    let entry = LogEntry {
        level: level.into(),
        message,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
    };
    if let Some(h) = app_handle.lock().unwrap().as_ref() {
        let _ = h.emit("log-entry", &entry);
    }
    let mut buf = log_buffer.lock().unwrap();
    buf.push(entry);
    if buf.len() > 1000 {
        let keep = buf.len() - 500;
        buf.drain(0..keep);
    }
}

pub struct YseState {
    pub store: Arc<dyn Storage>,
    pub config: Arc<RwLock<YseConfig>>,
    pub crypto_key: Arc<RwLock<Option<chacha20poly1305::Key>>>,
    pub sender: Arc<RwLock<Option<SmtpSender>>>,
    pub poller_running: Arc<std::sync::atomic::AtomicBool>,
    pub plugin_manager: Arc<PluginManager>,
    #[allow(dead_code)]
    pub router: Arc<Router>,
    pub log_buffer: Arc<std::sync::Mutex<Vec<LogEntry>>>,
    pub event_tx: EventSender,
    pub app_handle: Arc<std::sync::Mutex<Option<tauri::AppHandle>>>,
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
            config: Arc::new(RwLock::new(YseConfig::default())),
            crypto_key: Arc::new(RwLock::new(None)),
            sender: Arc::new(RwLock::new(None)),
            poller_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            plugin_manager,
            router,
            log_buffer: Arc::new(std::sync::Mutex::new(Vec::new())),
            event_tx,
            app_handle: Arc::new(std::sync::Mutex::new(None)),
        })
    }

    /// Load persisted config from DB, called from Tauri setup (tokio runtime available).
    pub async fn load_config(&self) {
        if let Ok(Some(json)) = self.store.get_config_value("config").await {
            if let Ok(cfg) = serde_json::from_str::<YseConfig>(&json) {
                let password = cfg.crypto_password.clone();
                let mut w = self.config.write().await;
                *w = cfg;
                drop(w);
                if !password.is_empty() {
                    let key = derive_key(&password).map_err(|e| e.to_string()).ok();
                    if let Some(k) = key {
                        *self.crypto_key.write().await = Some(k);
                        self.log("info", "crypto key restored from saved password".into());
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

        // Auto-derive crypto key if password changed
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

    /// Set up the plugin request handler so plugins can send/log via core.
    pub fn setup_plugin_handler(&self) {
        use yse_core::crypto::encrypt;
        use yse_core::plugin::protocol::PluginRequest;
        use tauri::Emitter;

        let store = self.store.clone();
        let config = self.config.clone();
        let crypto_key = self.crypto_key.clone();
        let sender = self.sender.clone();
        let log_buffer = self.log_buffer.clone();
        let app_handle = self.app_handle.clone();

        let handler: yse_core::plugin::process::PluginRequestHandler = Arc::new(move |req| {
            match req {
                PluginRequest::Send { from_addr, to_addr, text, .. } => {
                    let store = store.clone();
                    let config = config.clone();
                    let crypto_key = crypto_key.clone();
                    let sender = sender.clone();
                    let app_handle = app_handle.clone();
                    let log_buffer = log_buffer.clone();
                    tokio::spawn(async move {
                        let email_user = config.read().await.email_username.clone();
                        let msg = Message::new(from_addr.clone(), to_addr.clone(), text.clone());

                        let payload = match msg.to_json() {
                            Ok(p) => p,
                            Err(_) => {
                                log_emit(&app_handle, &log_buffer, "error", format!("plugin Send serialization failed"));
                                return;
                            }
                        };
                        let key = match crypto_key.read().await.as_ref() {
                            Some(k) => *k,
                            None => {
                                log_emit(&app_handle, &log_buffer, "warn", format!("plugin Send skipped: crypto key not set"));
                                return;
                            }
                        };
                        let encrypted = match encrypt(&key, &payload) {
                            Ok(e) => e,
                            Err(_) => {
                                log_emit(&app_handle, &log_buffer, "error", format!("plugin Send encrypt failed"));
                                return;
                            }
                        };

                        // Send via SMTP (envelope FROM must match authenticated user)
                        if let Some(s) = sender.read().await.as_ref() {
                            let d = disguise::disguise();
                            match s
                                .send((&email_user, &d.display_name), &email_user, encrypted, vec![])
                                .await
                            {
                                Ok(_) => {
                                    log_emit(&app_handle, &log_buffer, "info", format!("plugin Send delivered to {}", to_addr));
                                }
                                Err(e) => {
                                    log_emit(&app_handle, &log_buffer, "error", format!("plugin Send SMTP failed: {}", e));
                                }
                            }
                        }

                        let _ = store.save_message(&msg).await;
                        log_emit(&app_handle, &log_buffer, "info", format!("plugin Send saved msg {} to {}: {}", msg.id, to_addr, text.as_deref().unwrap_or("")));

                        // Notify frontend
                        if let Some(h) = app_handle.lock().unwrap().as_ref() {
                            let _ = h.emit("new-message", &msg);
                        }
                    });
                }
                PluginRequest::Log { level, msg } => {
                    log_emit(&app_handle, &log_buffer, &level, msg);
                }
            }
        });

        self.plugin_manager.set_request_handler(handler);
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

        // Push to frontend in real-time
        if let Some(handle) = self.app_handle.lock().unwrap().as_ref() {
            let _ = handle.emit("log-entry", &entry);
        }

        let mut buf = self.log_buffer.lock().unwrap();
        buf.push(entry);
        if buf.len() > 1000 {
            let keep = buf.len() - 500;
            buf.drain(0..keep);
        }
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
    let key = state.crypto_key.read().await;
    let key = key.as_ref().ok_or("crypto key not set")?;
    let own_addr = state.config.read().await.own_address.clone();
    let msg = Message::new(own_addr, to.clone(), Some(text));

    // Dispatch to plugins first (independent of SMTP availability)
    let email_username = {
        let cfg = state.config.read().await;
        let mappings: Vec<(String, String)> = cfg
            .plugin_mappings
            .iter()
            .map(|m| (m.virtual_addr.clone(), m.plugin_id.clone()))
            .collect();
        let email = cfg.email_username.clone();
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
        email
    };

    // Save message regardless of SMTP
    state.store.save_message(&msg).await.map_err(|e| e.to_string())?;

    // Send via SMTP (best-effort for external delivery)
    if let Some(sender) = state.sender.read().await.as_ref() {
        let payload = msg.to_json().map_err(|e| e.to_string())?;
        let encrypted = encrypt(key, &payload).map_err(|e| e.to_string())?;
        let d = disguise::disguise();
        if let Err(e) = sender
            .send(
                (&email_username, &d.display_name),
                &email_username,
                encrypted,
                vec![],
            )
            .await
        {
            state.log("error", format!("SMTP send failed: {}", e));
        }
    } else {
        state.log("warn", "SMTP not configured, message not sent via email".into());
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

impl YseState {
    /// Start IMAP polling. Called from Tauri command or auto-start.
    pub async fn start_polling_inner(
        &self,
        app_handle: tauri::AppHandle,
    ) -> Result<(), String> {
        use tauri::Emitter;

        let (imap_cfg, key, store, own_addr, mappings, plugin_manager) = {
            let cfg = self.config.read().await;
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

            let key = self
                .crypto_key
                .read()
                .await
                .clone()
                .ok_or("crypto key not set")?;

            (imap, key, self.store.clone(), self.config.read().await.own_address.clone(), mappings, self.plugin_manager.clone())
        };

        self.log("info", "IMAP polling started".into());

        let poller = ImapPoller::new(imap_cfg);

        tokio::spawn(async move {
            poller
                .run(move |raw_email| {
                    let parsed = match parse_incoming(&raw_email) {
                        Ok(p) => p,
                        Err(e) => {
                            let entry = LogEntry {
                                level: "warn".into(),
                                message: format!("IMAP parse failed: {}", e),
                                timestamp: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_millis() as u64,
                            };
                            let _ = app_handle.emit("log-entry", &entry);
                            return;
                        }
                    };
                    let entry = LogEntry {
                        level: "info".into(),
                        message: format!("IMAP received {} bytes, decrypting...", parsed.encrypted_body.len()),
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis() as u64,
                    };
                    let _ = app_handle.emit("log-entry", &entry);

                    let decrypted = match decrypt(&key, &parsed.encrypted_body) {
                        Ok(d) => d,
                        Err(e) => {
                            let entry = LogEntry {
                                level: "warn".into(),
                                message: format!("decrypt failed: {}", e),
                                timestamp: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_millis() as u64,
                            };
                            let _ = app_handle.emit("log-entry", &entry);
                            return;
                        }
                    };
                    let msg = match Message::from_json(&decrypted) {
                        Ok(m) => m,
                        Err(e) => {
                            let entry = LogEntry {
                                level: "warn".into(),
                                message: format!("JSON parse failed: {}", e),
                                timestamp: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_millis() as u64,
                            };
                            let _ = app_handle.emit("log-entry", &entry);
                            return;
                        }
                    };

                    let store = store.clone();
                    let handle = app_handle.clone();
                    let own = own_addr.clone();
                    let pm = plugin_manager.clone();
                    let mapping = mappings.clone();

                    tokio::spawn(async move {
                        let _ = store.save_message(&msg).await;

                        pm.dispatch_message(
                            &msg.to_addr,
                            &msg.from_addr,
                            msg.text.as_deref(),
                            msg.meta.as_ref(),
                            msg.files.as_ref(),
                            &mapping,
                        )
                        .await;

                        let entry = LogEntry {
                            level: "info".into(),
                            message: format!("received msg {} from {} to {}", msg.id, msg.from_addr, msg.to_addr),
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_millis() as u64,
                        };
                        let _ = handle.emit("log-entry", &entry);

                        if msg.to_addr == own || msg.from_addr == own {
                            let _ = handle.emit("new-message", &msg);
                        }
                    });
                })
                .await;
        });

        Ok(())
    }
}

#[tauri::command]
pub async fn start_polling(
    state: State<'_, YseState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    state.start_polling_inner(app_handle).await
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
    name: String,
    exec_path: String,
) -> Result<(), String> {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hasher);
    let id = format!("{:x}", hasher.finish());

    let pc = PluginConfig {
        id: id.clone(),
        name: name.clone(),
        exec_path: exec_path.clone(),
        args: vec![],
        enabled: true,
    };
    state.store.save_plugin(&pc).await.map_err(|e| e.to_string())?;
    state
        .plugin_manager
        .start_plugin(&id, &exec_path, &[])
        .await?;

    // Auto-create a contact mapping
    let vaddr = if name.contains('@') {
        name.clone()
    } else {
        format!("{}@yse.org", name)
    };
    {
        let mut cfg = state.config.write().await;
        if !cfg.plugin_mappings.iter().any(|m| m.virtual_addr == vaddr) {
            cfg.plugin_mappings.push(yse_core::config::PluginMapping {
                virtual_addr: vaddr.clone(),
                plugin_id: id.clone(),
            });
            // Persist to DB
            let json = serde_json::to_string(&*cfg).map_err(|e| e.to_string())?;
            state.store.set_config_value("config", &json).await.map_err(|e| e.to_string())?;
        }
    }

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
    let buf = state.log_buffer.lock().unwrap();
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
