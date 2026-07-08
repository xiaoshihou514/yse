use std::collections::HashMap;
use std::sync::Arc;
use tauri::{Emitter, State};
use tracing::warn;
use yse_core::logging::{log_buffer_new, log_buffer_push, LogBuffer, LogEntry};
use yse_core::{
    config::YseConfig,
    crypto::{decrypt, derive_key},
    disguise,
    email::{
        imap::{ImapConfig, ImapPoller},
        parser::parse_incoming,
        smtp::{SmtpConfig, SmtpSender},
    },
    event::CoreEvent,
    identity,
    message::Message,
    plugin::{
        process_manager::ProcessInfo,
    },
    store::PluginConfig,
};

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

/// Emit a log entry: push to Tauri event (if app_handle ready) + buffer.
fn log_emit(
    app_handle: &Arc<std::sync::Mutex<Option<tauri::AppHandle>>>,
    log_buffer: &LogBuffer,
    level: &str,
    message: String,
) {
    let entry = LogEntry::new(level, message);
    if let Some(h) = app_handle.lock().unwrap().as_ref() {
        let _ = h.emit("log-entry", &entry);
    }
    log_buffer_push(log_buffer, entry);
}

pub struct YseState {
    pub core: yse_core::app::CoreState,
    pub log_buffer: LogBuffer,
    pub app_handle: Arc<std::sync::Mutex<Option<tauri::AppHandle>>>,
}

impl YseState {
    pub fn new(db_path: std::path::PathBuf) -> Result<Self, String> {
        Ok(Self {
            core: yse_core::app::CoreState::new(db_path, "me")?,
            log_buffer: log_buffer_new(),
            app_handle: Arc::new(std::sync::Mutex::new(None)),
        })
    }

    /// Load persisted config from DB, called from Tauri setup (tokio runtime available).
    pub async fn load_config(&self) {
        if let Ok(Some(json)) = self.core.store.get_config_value("config").await {
            if let Ok(cfg) = serde_json::from_str::<YseConfig>(&json) {
                let password = cfg.crypto_password.clone();
                let own_name = cfg.own_address.clone();
                let mut w = self.core.config.write().await;
                *w = cfg;
                drop(w);

                self.core.session_registry.set_local_name(&own_name);

                // Load contact hashes into session registry
                if let Ok(hashes) = self.core.store.get_contact_hashes().await {
                    let map: HashMap<String, String> = hashes.into_iter().collect();
                    self.core.session_registry.set_contact_hashes(map);
                }

                if !password.is_empty() {
                    let key = derive_key(&password).map_err(|e| e.to_string()).ok();
                    if let Some(k) = key {
                        *self.core.crypto_key.write().await = Some(k);
                        self.log("info", "crypto key restored from saved password".into());
                    }
                }
                // Initialize SMTP sender so it's ready without re-saving
                {
                    let cfg = self.core.config.read().await;
                    let smtp_cfg = SmtpConfig {
                        server: cfg.email_smtp_server.clone(),
                        port: cfg.email_smtp_port,
                        username: cfg.email_username.clone(),
                        password: cfg.email_password.clone(),
                    };
                    if !smtp_cfg.server.is_empty() && !smtp_cfg.username.is_empty() {
                        *self.core.sender.write().await = Some(SmtpSender::new(smtp_cfg));
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
        *self.core.sender.write().await = Some(SmtpSender::new(smtp_cfg));

        // Auto-derive crypto key if password changed
        if !cfg.crypto_password.is_empty() {
            let new_key = derive_key(&cfg.crypto_password).map_err(|e| e.to_string())?;
            *self.core.crypto_key.write().await = Some(new_key);
        }

        self.core.store
            .set_config_value("config", &serde_json::to_string(&cfg).unwrap())
            .await
            .map_err(|e| e.to_string())?;

        *self.core.config.write().await = cfg;
        Ok(())
    }

    /// Set up the plugin request handler so plugins can send/log via core.
    pub fn setup_plugin_handler(&self) {
        use tauri::Emitter;
        use yse_core::crypto::encrypt;
        use yse_core::plugin::protocol::PluginRequest;

        let store = self.core.store.clone();
        let config = self.core.config.clone();
        let crypto_key = self.core.crypto_key.clone();
        let sender = self.core.sender.clone();
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

                        // Save locally BEFORE sending via SMTP, so the IMAP poll finds
                        // this message already in the DB and skips re-routing.
                        let _ = store.save_message(&msg).await;

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
                            Some(crypto_key) => *crypto_key,
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
                        if let Some(sender) = sender.read().await.as_ref() {
                            let disguised = disguise::disguise();
                            match sender
                                .send(
                                    (&email_user, &disguised.display_name),
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

        self.core.process_manager.set_request_handler(handler);
    }

    pub fn log(&self, level: &str, message: String) {
        let entry = LogEntry::new(level, message);
        let _ = self.core.event_tx.send(CoreEvent::Log {
            level: entry.level.clone(),
            message: entry.message.clone(),
        });

        // Push to frontend in real-time
        if let Some(handle) = self.app_handle.lock().unwrap().as_ref() {
            let _ = handle.emit("log-entry", &entry);
        }

        log_buffer_push(&self.log_buffer, entry);
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
    // Format sender address using session registry (name#hash@hostname)
    let from_addr = state.core.session_registry.format_sender_address(&to);

    let msg = match meta {
        Some(m) => Message::new(from_addr, to.clone(), Some(text)).with_meta(m),
        None => Message::new(from_addr, to.clone(), Some(text)),
    };

    // Save message
    state.core.store
        .save_message(&msg)
        .await
        .map_err(|e| e.to_string())?;

    // Route to local plugin if addressed to this machine
    let plugin_configs = state.core.store.list_plugins().await.unwrap_or_default();
    let _ = state.core.session_registry
        .route(
            &msg.to_addr,
            &msg.from_addr,
            msg.text.as_deref(),
            msg.meta.as_ref(),
            msg.files.as_deref(),
            &plugin_configs,
            &state.core.process_manager,
        )
        .await;

    // Send via SMTP — fail the command if delivery fails
    if let Err(e) = state.core.send_encrypted(&msg).await {
        state.log("error", format!("SMTP send failed: {}", e));
        return Err(format!("SMTP 发送失败: {}", e));
    }

    // Only mark processed after successful send (prevents IMAP re-route of local copy)
    let _ = state.core.store.mark_processed(&msg.id).await;

    state.log("info", format!("sent message to {}", to));
    Ok(())
}

#[tauri::command]
pub async fn get_messages(
    state: State<'_, YseState>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<Message>, String> {
    state.core.store
        .list_messages(limit.unwrap_or(50), offset.unwrap_or(0))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_config(state: State<'_, YseState>) -> Result<YseConfig, String> {
    Ok(state.core.config.read().await.clone())
}

#[tauri::command]
pub async fn save_config(state: State<'_, YseState>, config: YseConfig) -> Result<(), String> {
    state.update_config(config).await
}

impl YseState {
    /// Start IMAP polling. Called from Tauri command or auto-start.
    pub async fn start_polling_inner(&self, app_handle: tauri::AppHandle) -> Result<(), String> {
        use tauri::Emitter;

        let (imap_cfg, key, store, own_addr, session_registry, process_manager) = {
            let cfg = self.core.config.read().await;
            let imap = ImapConfig {
                server: cfg.email_imap_server.clone(),
                port: cfg.email_imap_port,
                username: cfg.email_username.clone(),
                password: cfg.email_password.clone(),
            };
            drop(cfg);

            let key = {
                let guard = self.core.crypto_key.read().await;
                guard.as_ref().copied().ok_or("crypto key not set")?
            };

            (
                imap,
                key,
                self.core.store.clone(),
                self.core.config.read().await.own_address.clone(),
                self.core.session_registry.clone(),
                self.core.process_manager.clone(),
            )
        };

        self.log("info", "IMAP polling started".into());
        self.core
            .poller_running
            .store(true, std::sync::atomic::Ordering::SeqCst);

        let last_uid = store
            .get_config_value("imap_last_uid")
            .await
            .ok()
            .flatten()
            .and_then(|s| s.parse().ok());

        let state_app_handle = self.app_handle.clone();
        let state_log_buffer = self.log_buffer.clone();
        let save_store = store.clone();

        let core_clone = self.core.clone();
        let poller_flag = self.core.poller_running.clone();
        tokio::spawn(async move {
            let mut poller = ImapPoller::new(imap_cfg, last_uid);
            poller.set_running_flag(poller_flag);

            let last_uid_arc = poller.last_uid_arc();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                    let uid = *last_uid_arc.lock().unwrap();
                    if let Some(u) = uid {
                        let _ = save_store
                            .set_config_value("imap_last_uid", &u.to_string())
                            .await;
                    }
                }
            });

            poller
                .run_with_log(
                    move |raw_email| {
                        let parsed = match parse_incoming(&raw_email) {
                            Ok(p) => p,
                            Err(e) => {
                                let entry = LogEntry::new("warn", format!("IMAP parse failed: {}", e));
                                let _ = app_handle.emit("log-entry", &entry);
                                return;
                            }
                        };
                        let entry = LogEntry::new(
                            "debug",
                            format!(
                                "IMAP received {} bytes, decrypting...",
                                parsed.encrypted_body.len()
                            ),
                        );
                        let _ = app_handle.emit("log-entry", &entry);

                        let decrypted = match decrypt(&key, &parsed.encrypted_body) {
                            Ok(d) => d,
                            Err(e) => {
                                let entry = LogEntry::new("warn", format!("decrypt failed: {}", e));
                                let _ = app_handle.emit("log-entry", &entry);
                                return;
                            }
                        };
                        let msg = match Message::from_json(&decrypted) {
                            Ok(m) => m,
                            Err(e) => {
                                let entry = LogEntry::new("warn", format!("JSON parse failed: {}", e));
                                let _ = app_handle.emit("log-entry", &entry);
                                return;
                            }
                        };

                        let store = store.clone();
                        let handle = app_handle.clone();
                        let own = own_addr.clone();
                        let sr = session_registry.clone();
                        let pm = process_manager.clone();
                        let core_clone = core_clone.clone();

                        tokio::spawn(async move {
                            let s: &dyn yse_core::store::Storage = &*store;
                            let result = yse_core::imap_ingest::ingest_message(
                                &msg, s, &own, &sr, &pm,
                            ).await;

                            let Some(ref route_result) = result.route_result else {
                                let entry = LogEntry::new(
                                    "debug",
                                    format!("received msg {} from {} to {}", msg.id, msg.from_addr, msg.to_addr),
                                );
                                let _ = handle.emit("log-entry", &entry);
                                if result.show_in_chat {
                                    let _ = handle.emit("new-message", &msg);
                                }
                                return;
                            };

                            // Error reply for plugin-not-found
                            if let yse_core::plugin::session::RouteResult::PluginNotFound { plugin_name } = route_result {
                                let plugin_configs = store.list_plugins().await.unwrap_or_default();
                                let available: Vec<String> = plugin_configs.iter()
                                    .filter(|p| p.enabled)
                                    .map(|p| p.name.clone())
                                    .collect();
                                let reply_text = if available.is_empty() {
                                    format!("错误: 插件「{}」在此机器上不存在。\n\n当前没有可用插件。", plugin_name)
                                } else {
                                    format!(
                                        "错误: 插件「{}」在此机器上不存在。\n\n可用插件: {}",
                                        plugin_name, available.join(", ")
                                    )
                                };
                                let reply_msg = Message::new(own.to_string(), msg.from_addr.clone(), Some(reply_text));
                                if let Err(e) = core_clone.send_encrypted(&reply_msg).await {
                                    warn!("send_plugin_error_reply failed: {}", e);
                                }
                            }

                            // Log
                            let log_msg = match route_result {
                                yse_core::plugin::session::RouteResult::Dispatched =>
                                    format!("session-routed msg {} via session registry", msg.id),
                                _ => format!("msg {}: not routed locally ({:?})", msg.id, route_result),
                            };
                            let log = LogEntry::new("info", log_msg);
                            let _ = handle.emit("log-entry", &log);

                            let entry = LogEntry::new(
                                "debug",
                                format!("received msg {} from {} to {}", msg.id, msg.from_addr, msg.to_addr),
                            );
                            let _ = handle.emit("log-entry", &entry);

                            if result.show_in_chat {
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
        let plugins = match self.core.store.list_plugins().await {
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
    state.core.poller_running
        .store(false, std::sync::atomic::Ordering::SeqCst);
    state.log("info", "IMAP polling stopped".into());
    Ok(())
}

#[tauri::command]
pub async fn list_plugins(state: State<'_, YseState>) -> Result<Vec<PluginConfig>, String> {
    state.core.store.list_plugins().await.map_err(|e| e.to_string())
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
    state.core.store
        .save_plugin(&pc)
        .await
        .map_err(|e| e.to_string())?;

    state.core.process_manager
        .start(&id, &name, &exec_path, &[])
        .await?;

    state.log("info", format!("plugin added: {}", id));
    Ok(())
}

#[tauri::command]
pub async fn remove_plugin(state: State<'_, YseState>, id: String) -> Result<(), String> {
    let _ = state.core.process_manager.stop(&id).await;
    state.core.store
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
        let plugins = state.core.store
            .list_plugins()
            .await
            .map_err(|e| e.to_string())?;
        let p = plugins
            .iter()
            .find(|p| p.id == id)
            .ok_or("plugin not found")?;
        state.core.process_manager
            .start(&id, &p.name, &p.exec_path, &p.args)
            .await?;
    } else {
        state.core.process_manager.stop(&id).await?;
    }
    state.core.store
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
    let plugins = state.core.store
        .list_plugins()
        .await
        .map_err(|e| e.to_string())?;
    let p = plugins
        .iter()
        .find(|p| p.id == id)
        .ok_or("plugin not found")?;
    state.core.process_manager
        .start(&id, &p.name, &p.exec_path, &p.args)
        .await?;
    state.log("info", format!("plugin started: {}", id));
    Ok(())
}

#[tauri::command]
pub async fn stop_plugin(state: State<'_, YseState>, id: String) -> Result<(), String> {
    match state.core.process_manager.stop(&id).await {
        Ok(_) => state.log("info", format!("plugin stopped: {}", id)),
        Err(_) => state.log("info", format!("plugin {} not running, skipping", id)),
    }
    Ok(())
}

#[tauri::command]
pub async fn list_running_plugins(state: State<'_, YseState>) -> Result<Vec<String>, String> {
    Ok(state.core.process_manager.running_ids().await)
}

#[tauri::command]
pub async fn list_processes(state: State<'_, YseState>) -> Result<Vec<ProcessInfo>, String> {
    Ok(state.core.process_manager.list_all().await)
}

#[tauri::command]
pub async fn list_sessions(state: State<'_, YseState>) -> Result<Vec<yse_core::plugin::session::Session>, String> {
    Ok(state.core.session_registry.list_sessions().await)
}

#[tauri::command]
pub async fn get_hostname() -> Result<String, String> {
    Ok(identity::local_hostname())
}

#[tauri::command]
pub async fn set_local_hostname(
    state: State<'_, YseState>,
    hostname: String,
) -> Result<(), String> {
    state.core.session_registry.set_local_hostname(&hostname);
    Ok(())
}

#[tauri::command]
pub async fn toggle_hide_conversation(
    state: State<'_, YseState>,
    address: String,
    hidden: bool,
) -> Result<(), String> {
    state.core.store
        .set_hidden(&address, hidden)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_conversation(
    state: State<'_, YseState>,
    address: String,
) -> Result<(), String> {
    state.core.store
        .delete_messages_for_address(&address)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_hidden_addresses(state: State<'_, YseState>) -> Result<Vec<String>, String> {
    state.core.store
        .get_hidden_addresses()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_contact_hashes(
    state: State<'_, YseState>,
) -> Result<Vec<(String, String)>, String> {
    state.core.store
        .get_contact_hashes()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_known_hostnames(state: State<'_, YseState>) -> Result<Vec<String>, String> {
    let addrs = state.core.store
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
    }, None);
    let _session = p
        .connect_sync()
        .map_err(|e| format!("IMAP 连接失败: {}", e))?;
    Ok("邮箱连接正常".into())
}
