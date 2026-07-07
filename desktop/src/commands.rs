use std::collections::HashMap;
use std::sync::Arc;
use tauri::{Emitter, State};
use tokio::sync::RwLock;
use tracing::warn;
use yse_core::crypto::Key;
use yse_core::{
    config::YseConfig,
    crypto::{decrypt, derive_key, encrypt},
    disguise,
    email::{
        imap::{ImapConfig, ImapPoller},
        parser::parse_incoming,
        smtp::{SmtpConfig, SmtpSender},
    },
    event::{CoreEvent, EventSender    },
    identity,
    message::Message,
    plugin::{
        process_manager::{PluginProcessManager, ProcessInfo},
        session::SessionRegistry,
    },
    store::{sqlite::SqliteStorage, PluginConfig, Storage},
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
    pub process_manager: Arc<PluginProcessManager>,
    pub session_registry: Arc<SessionRegistry>,
    pub log_buffer: Arc<std::sync::Mutex<Vec<LogEntry>>>,
    pub event_tx: EventSender,
    pub app_handle: Arc<std::sync::Mutex<Option<tauri::AppHandle>>>,
}

impl YseState {
    pub fn new(db_path: std::path::PathBuf) -> Result<Self, String> {
        let store: Arc<dyn Storage> =
            Arc::new(SqliteStorage::open(db_path).map_err(|e| e.to_string())?);
        let (event_tx, _) = yse_core::event::event_channel();
        let process_manager = Arc::new(PluginProcessManager::new());
        let session_registry = Arc::new(SessionRegistry::new("me"));

        Ok(Self {
            store,
            config: Arc::new(RwLock::new(YseConfig::default())),
            crypto_key: Arc::new(RwLock::new(None)),
            sender: Arc::new(RwLock::new(None)),
            poller_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            process_manager,
            session_registry,
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
                let own_name = cfg.own_address.clone();
                let mut w = self.config.write().await;
                *w = cfg;
                drop(w);

                self.session_registry.set_local_name(&own_name);

                // Load contact hashes into session registry
                if let Ok(hashes) = self.store.get_contact_hashes().await {
                    let map: HashMap<String, String> = hashes.into_iter().collect();
                    self.session_registry.set_contact_hashes(map);
                }

                if !password.is_empty() {
                    let key = derive_key(&password).map_err(|e| e.to_string()).ok();
                    if let Some(k) = key {
                        *self.crypto_key.write().await = Some(k);
                        self.log("info", "crypto key restored from saved password".into());
                    }
                }
                // Initialize SMTP sender so it's ready without re-saving
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
                        self.log("info", "SMTP sender initialized from saved config".into());
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
        use tauri::Emitter;
        use yse_core::crypto::encrypt;
        use yse_core::plugin::protocol::PluginRequest;

        let store = self.store.clone();
        let config = self.config.clone();
        let crypto_key = self.crypto_key.clone();
        let sender = self.sender.clone();
        let log_buffer = self.log_buffer.clone();
        let app_handle = self.app_handle.clone();

        let handler: yse_core::plugin::process::PluginRequestHandler = Arc::new(move |req| {
            match req {
                PluginRequest::Send {
                    from_addr,
                    to_addr,
                    text,
                    ..
                } => {
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
                                log_emit(
                                    &app_handle,
                                    &log_buffer,
                                    "error",
                                    "plugin Send serialization failed".to_string(),
                                );
                                return;
                            }
                        };
                        let key = match crypto_key.read().await.as_ref() {
                            Some(k) => *k,
                            None => {
                                log_emit(
                                    &app_handle,
                                    &log_buffer,
                                    "warn",
                                    "plugin Send skipped: crypto key not set".to_string(),
                                );
                                return;
                            }
                        };
                        let encrypted = match encrypt(&key, &payload) {
                            Ok(e) => e,
                            Err(_) => {
                                log_emit(
                                    &app_handle,
                                    &log_buffer,
                                    "error",
                                    "plugin Send encrypt failed".to_string(),
                                );
                                return;
                            }
                        };

                        // Send via SMTP (envelope FROM must match authenticated user)
                        if let Some(s) = sender.read().await.as_ref() {
                            let d = disguise::disguise();
                            match s
                                .send(
                                    (&email_user, &d.display_name),
                                    &email_user,
                                    encrypted,
                                    vec![],
                                )
                                .await
                            {
                                Ok(_) => {
                                    log_emit(
                                        &app_handle,
                                        &log_buffer,
                                        "info",
                                        format!("plugin Send delivered to {}", to_addr),
                                    );
                                }
                                Err(e) => {
                                    log_emit(
                                        &app_handle,
                                        &log_buffer,
                                        "error",
                                        format!("plugin Send SMTP failed: {}", e),
                                    );
                                }
                            }
                        }

                        let _ = store.save_message(&msg).await;
                        log_emit(
                            &app_handle,
                            &log_buffer,
                            "info",
                            format!(
                                "plugin Send saved msg {} to {}: {}",
                                msg.id,
                                to_addr,
                                text.as_deref().unwrap_or("")
                            ),
                        );

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

        self.process_manager.set_request_handler(handler);
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
    meta: Option<serde_json::Value>,
) -> Result<(), String> {
    let key = state.crypto_key.read().await;
    let key = key.as_ref().ok_or("crypto key not set")?;

    let email_username = state.config.read().await.email_username.clone();

    // Format sender address using session registry (name#hash@hostname)
    let from_addr = state.session_registry.format_sender_address(&to);

    let msg = match meta {
        Some(m) => Message::new(from_addr, to.clone(), Some(text)).with_meta(m),
        None => Message::new(from_addr, to.clone(), Some(text)),
    };

    // Save message
    state
        .store
        .save_message(&msg)
        .await
        .map_err(|e| e.to_string())?;

    // Route to local plugin if addressed to this machine
    let plugin_configs = state.store.list_plugins().await.unwrap_or_default();
    let _ = state
        .session_registry
        .route(
            &msg.to_addr,
            &msg.from_addr,
            msg.text.as_deref(),
            msg.meta.as_ref(),
            msg.files.as_ref(),
            &plugin_configs,
            &state.process_manager,
        )
        .await;

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
        state.log(
            "warn",
            "SMTP not configured, message not sent via email".into(),
        );
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
    pub async fn start_polling_inner(&self, app_handle: tauri::AppHandle) -> Result<(), String> {
        use tauri::Emitter;

        let sender = self.sender.clone();
        let crypto_key = self.crypto_key.clone();
        let (imap_cfg, key, store, own_addr, session_registry, process_manager, yse_cfg) = {
            let cfg = self.config.read().await;
            let imap = ImapConfig {
                server: cfg.email_imap_server.clone(),
                port: cfg.email_imap_port,
                username: cfg.email_username.clone(),
                password: cfg.email_password.clone(),
            };
            drop(cfg);

            let key = {
                let guard = self.crypto_key.read().await;
                guard.as_ref().copied().ok_or("crypto key not set")?
            };

            (
                imap,
                key,
                self.store.clone(),
                self.config.read().await.own_address.clone(),
                self.session_registry.clone(),
                self.process_manager.clone(),
                self.config.clone(),
            )
        };

        self.log("info", "IMAP polling started".into());
        self.poller_running
            .store(true, std::sync::atomic::Ordering::SeqCst);

        let mut poller = ImapPoller::new(imap_cfg);
        poller.set_running_flag(self.poller_running.clone());
        let state_app_handle = self.app_handle.clone();
        let state_log_buffer = self.log_buffer.clone();

        tokio::spawn(async move {
            poller
                .run_with_log(
                    move |raw_email| {
                        let parsed = match parse_incoming(&raw_email) {
                            Ok(p) => p,
                            Err(e) => {
                                let entry = LogEntry {
                                    level: "warn".into(),
                                    message: format!("IMAP parse failed: {}", e),
                                    timestamp: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap()
                                        .as_millis()
                                        as u64,
                                };
                                let _ = app_handle.emit("log-entry", &entry);
                                return;
                            }
                        };
                        let entry = LogEntry {
                            level: "debug".into(),
                            message: format!(
                                "IMAP received {} bytes, decrypting...",
                                parsed.encrypted_body.len()
                            ),
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
                                        .as_millis()
                                        as u64,
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
                                        .as_millis()
                                        as u64,
                                };
                                let _ = app_handle.emit("log-entry", &entry);
                                return;
                            }
                        };

                        let store = store.clone();
                        let handle = app_handle.clone();
                        let own = own_addr.clone();
                        let sr = session_registry.clone();
                        let pm = process_manager.clone();
                        let cfg = yse_cfg.clone();
                        let snd = sender.clone();
                        let ck = crypto_key.clone();

                        tokio::spawn(async move {
                            let _ = store.save_message(&msg).await;

                            let already = store.is_processed(&msg.id).await.unwrap_or(false);
                            if !already {
                                let plugin_configs = store.list_plugins().await.unwrap_or_default();
                                // Route via session registry (starts plugin if needed)
                                let result = sr
                                    .route(
                                        &msg.to_addr,
                                        &msg.from_addr,
                                        msg.text.as_deref(),
                                        msg.meta.as_ref(),
                                        msg.files.as_ref(),
                                        &plugin_configs,
                                        &pm,
                                    )
                                    .await;
                                let _ = store.mark_processed(&msg.id).await;

                                // If the message was for a plugin that doesn't exist here,
                                // send a helpful error reply back to the sender.
                                if matches!(&result, yse_core::plugin::session::RouteResult::PluginNotFound { .. }) {
                                    if let Some(plugin_name) = result.plugin_name() {
                                        let reply_text = format!(
                                            "错误: 插件「{}」在此机器上不存在。\n\n\
                                             可用插件列表可通过 /help 查看。\n\
                                             请联系管理员添加所需插件。",
                                            plugin_name
                                        );
                                        let ck_guard = ck.read().await;
                                        if let Some(ref k) = *ck_guard {
                                            if let Err(e) = send_plugin_error_reply(
                                                &msg.from_addr, &own, &reply_text,
                                                &*snd, k, &cfg,
                                            ).await {
                                                warn!("send_plugin_error_reply failed: {}", e);
                                            }
                                        }
                                    }
                                }

                                let log_msg = match &result {
                                    yse_core::plugin::session::RouteResult::Dispatched =>
                                        format!("session-routed msg {} via session registry", msg.id),
                                    _ => format!("msg {}: not routed locally ({:?})", msg.id, result),
                                };
                                let log = LogEntry {
                                    level: "info".into(),
                                    message: log_msg,
                                    timestamp: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap()
                                        .as_millis() as u64,
                                };
                                let _ = handle.emit("log-entry", &log);
                            }

                            let entry = LogEntry {
                                level: "debug".into(),
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
                    },
                    Arc::new(move |level, msg| {
                        log_emit(&state_app_handle, &state_log_buffer, level, msg);
                    }),
                )
                .await;
        });

        Ok(())
    }

    /// Load plugin configs into memory (no auto-start).
    /// Plugins are started on demand by SessionRegistry when messages arrive.
    pub async fn auto_start_plugins(&self) {
        let plugins = match self.store.list_plugins().await {
            Ok(p) => p,
            Err(_) => return,
        };
        let count = plugins.iter().filter(|p| p.enabled).count();
        self.log(
            "info",
            format!("loaded {} enabled plugin config(s), no auto-start", count),
        );
    }
}

/// Send a reply message to the original sender when a plugin is not found.
/// This lets the sender know what happened instead of getting a silent failure.
async fn send_plugin_error_reply(
    to_addr: &str,
    from_addr: &str,
    reply_text: &str,
    sender: &tokio::sync::RwLock<Option<SmtpSender>>,
    crypto_key: &Key,
    cfg: &tokio::sync::RwLock<YseConfig>,
) -> Result<(), String> {
    let email_user = cfg.read().await.email_username.clone();
    let reply_msg = Message::new(from_addr.to_string(), to_addr.to_string(), Some(reply_text.to_string()));
    let payload = reply_msg.to_json().map_err(|e| e.to_string())?;
    let encrypted = encrypt(crypto_key, &payload).map_err(|e| e.to_string())?;
    if let Some(s) = sender.read().await.as_ref() {
        let d = disguise::disguise();
        s.send((&email_user, &d.display_name), &email_user, encrypted, vec![])
            .await
            .map_err(|e| format!("SMTP send error reply failed: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn start_polling(
    state: State<'_, YseState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    state.log("info", "start_polling command called from frontend".into());
    state.start_polling_inner(app_handle).await
}

#[tauri::command]
pub async fn auto_start_plugins(state: State<'_, YseState>) -> Result<(), String> {
    state.auto_start_plugins().await;
    state.log("info", "auto_start_plugins completed".into());
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
pub async fn list_plugins(state: State<'_, YseState>) -> Result<Vec<PluginConfig>, String> {
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
    state
        .store
        .save_plugin(&pc)
        .await
        .map_err(|e| e.to_string())?;

    state
        .process_manager
        .start(&id, &name, &exec_path, &[])
        .await?;

    state.log("info", format!("plugin added: {}", id));
    Ok(())
}

#[tauri::command]
pub async fn remove_plugin(state: State<'_, YseState>, id: String) -> Result<(), String> {
    let _ = state.process_manager.stop(&id).await;
    state
        .store
        .delete_plugin(&id)
        .await
        .map_err(|e| e.to_string())?;
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
        let plugins = state
            .store
            .list_plugins()
            .await
            .map_err(|e| e.to_string())?;
        let p = plugins
            .iter()
            .find(|p| p.id == id)
            .ok_or("plugin not found")?;
        state
            .process_manager
            .start(&id, &p.name, &p.exec_path, &p.args)
            .await?;
    } else {
        state.process_manager.stop(&id).await?;
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
pub async fn start_plugin(state: State<'_, YseState>, id: String) -> Result<(), String> {
    let plugins = state
        .store
        .list_plugins()
        .await
        .map_err(|e| e.to_string())?;
    let p = plugins
        .iter()
        .find(|p| p.id == id)
        .ok_or("plugin not found")?;
    state
        .process_manager
        .start(&id, &p.name, &p.exec_path, &p.args)
        .await?;
    state.log("info", format!("plugin started: {}", id));
    Ok(())
}

#[tauri::command]
pub async fn stop_plugin(state: State<'_, YseState>, id: String) -> Result<(), String> {
    match state.process_manager.stop(&id).await {
        Ok(_) => state.log("info", format!("plugin stopped: {}", id)),
        Err(_) => state.log("info", format!("plugin {} not running, skipping", id)),
    }
    Ok(())
}

#[tauri::command]
pub async fn list_running_plugins(state: State<'_, YseState>) -> Result<Vec<String>, String> {
    Ok(state.process_manager.running_ids().await)
}

#[tauri::command]
pub async fn list_processes(state: State<'_, YseState>) -> Result<Vec<ProcessInfo>, String> {
    Ok(state.process_manager.list_all().await)
}

#[tauri::command]
pub async fn list_sessions(state: State<'_, YseState>) -> Result<Vec<yse_core::plugin::session::Session>, String> {
    Ok(state.session_registry.list_sessions().await)
}

#[tauri::command]
pub async fn get_hostname() -> Result<String, String> {
    Ok(identity::local_hostname())
}

#[tauri::command]
pub async fn toggle_hide_conversation(
    state: State<'_, YseState>,
    address: String,
    hidden: bool,
) -> Result<(), String> {
    state
        .store
        .set_hidden(&address, hidden)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_conversation(
    state: State<'_, YseState>,
    address: String,
) -> Result<(), String> {
    state
        .store
        .delete_messages_for_address(&address)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_hidden_addresses(state: State<'_, YseState>) -> Result<Vec<String>, String> {
    state
        .store
        .get_hidden_addresses()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_contact_hashes(
    state: State<'_, YseState>,
) -> Result<Vec<(String, String)>, String> {
    state
        .store
        .get_contact_hashes()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_known_hostnames(state: State<'_, YseState>) -> Result<Vec<String>, String> {
    let addrs = state
        .store
        .get_unique_addresses()
        .await
        .map_err(|e| e.to_string())?;
    let mut hostnames: Vec<String> = addrs
        .iter()
        .filter_map(|a| identity::extract_hostname(a).map(|h| h.to_string()))
        .collect();
    hostnames.sort();
    hostnames.dedup();
    // Always include local hostname
    let local = identity::local_hostname();
    if !hostnames.contains(&local) {
        hostnames.push(local);
    }
    Ok(hostnames)
}

#[tauri::command]
pub async fn get_logs(
    state: State<'_, YseState>,
    limit: Option<usize>,
) -> Result<Vec<LogEntry>, String> {
    let buf = state.log_buffer.lock().unwrap();
    let limit = limit.unwrap_or(100);
    let start = if buf.len() > limit {
        buf.len() - limit
    } else {
        0
    };
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
    let _session = p
        .connect_sync()
        .map_err(|e| format!("IMAP 连接失败: {}", e))?;
    Ok("邮箱连接正常".into())
}
