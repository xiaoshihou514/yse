use std::sync::Arc;
use tauri::{Emitter, State};
use yse_core::{
    app::CoreState,
    config::YseConfig,
    disguise,
    email::imap::{ImapConfig, ImapPoller},
    identity,
    message::Message,
};

#[derive(serde::Serialize, Clone)]
pub struct LogEntry {
    pub level: String,
    pub message: String,
    pub timestamp: u64,
}

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

pub struct AppState {
    pub core: CoreState,
    pub log_buffer: Arc<std::sync::Mutex<Vec<LogEntry>>>,
    pub app_handle: Arc<std::sync::Mutex<Option<tauri::AppHandle>>>,
}

impl AppState {
    pub fn new(db_path: impl AsRef<std::path::Path>) -> Result<Self, String> {
        let core = CoreState::new(db_path, "me")?;
        Ok(Self {
            core,
            log_buffer: Arc::new(std::sync::Mutex::new(Vec::new())),
            app_handle: Arc::new(std::sync::Mutex::new(None)),
        })
    }

    pub fn log(&self, level: &str, message: String) {
        log_emit(&self.app_handle, &self.log_buffer, level, message);
    }
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<YseConfig, String> {
    Ok(state.core.config.read().await.clone())
}

#[tauri::command]
pub async fn save_config(state: State<'_, AppState>, config: YseConfig) -> Result<(), String> {
    state.core.update_config(config).await
}

#[tauri::command]
pub async fn get_hostname() -> Result<String, String> {
    Ok(identity::local_hostname())
}

#[tauri::command]
pub async fn get_messages(
    state: State<'_, AppState>,
    limit: u32,
) -> Result<Vec<Message>, String> {
    state
        .core
        .store
        .list_messages(limit, 0)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn send_message(
    state: State<'_, AppState>,
    to: String,
    text: String,
    _files: Option<Vec<String>>,
    meta: Option<serde_json::Value>,
) -> Result<(), String> {
    let key = state.core.crypto_key.read().await;
    let key = key.as_ref().ok_or("crypto key not set")?;

    let email_username = state.core.config.read().await.email_username.clone();

    let from_addr = state.core.session_registry.format_sender_address(&to);

    let msg = match meta {
        Some(m) => Message::new(from_addr, to.clone(), Some(text)).with_meta(m),
        None => Message::new(from_addr, to.clone(), Some(text)),
    };

    state
        .core
        .store
        .save_message(&msg)
        .await
        .map_err(|e| e.to_string())?;

    // Notify frontend immediately so the sent message shows up
    if let Some(h) = state.app_handle.lock().unwrap().as_ref() {
        let _ = h.emit("new-message", &msg);
    }

    if let Some(sender) = state.core.sender.read().await.as_ref() {
        let payload = msg.to_json().map_err(|e| e.to_string())?;
        let encrypted = yse_core::crypto::encrypt(key, &payload).map_err(|e| e.to_string())?;
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
pub async fn start_polling(
    state: State<'_, AppState>,
    _app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let (imap_cfg, key, store) = {
        let cfg = state.core.config.read().await;
        let imap = ImapConfig {
            server: cfg.email_imap_server.clone(),
            port: cfg.email_imap_port,
            username: cfg.email_username.clone(),
            password: cfg.email_password.clone(),
        };
        let key = state.core.crypto_key.read().await;
        let key = key.as_ref().copied().ok_or("crypto key not set")?;
        (imap, key, state.core.store.clone())
    };

    state
        .core
        .poller_running
        .store(true, std::sync::atomic::Ordering::SeqCst);

    let poller_flag = state.core.poller_running.clone();
    let ah1 = state.app_handle.clone();
    let lb1 = state.log_buffer.clone();
    let ah2 = state.app_handle.clone();
    let lb2 = state.log_buffer.clone();

    tokio::spawn(async move {
        let mut poller = ImapPoller::new(imap_cfg);
        poller.set_running_flag(poller_flag);
        poller
            .run_with_log(
                move |raw_email| {
                    let parsed = match yse_core::email::parser::parse_incoming(&raw_email) {
                        Ok(p) => p,
                        Err(e) => {
                            log_emit(
                                &ah1,
                                &lb1,
                                "warn",
                                format!("IMAP parse failed: {}", e),
                            );
                            return;
                        }
                    };
                    let decrypted = match yse_core::crypto::decrypt(&key, &parsed.encrypted_body) {
                        Ok(d) => d,
                        Err(e) => {
                            log_emit(
                                &ah1,
                                &lb1,
                                "error",
                                format!("decrypt failed: {}", e),
                            );
                            return;
                        }
                    };

                    let msg = match Message::from_json(&decrypted) {
                        Ok(m) => m,
                        Err(e) => {
                            log_emit(
                                &ah1,
                                &lb1,
                                "error",
                                format!("message parse failed: {}", e),
                            );
                            return;
                        }
                    };

                    let rt = tokio::runtime::Runtime::new().unwrap();
                    let processed = rt.block_on(async {
                        if let Ok(false) = store.is_processed(&msg.id).await {
                            let _ = store.mark_processed(&msg.id).await;
                            let _ = store.save_message(&msg).await;
                            true
                        } else {
                            false
                        }
                    });

                    if processed {
                        if let Some(h) = ah1.lock().unwrap().as_ref() {
                            let _ = h.emit("new-message", &msg);
                        }
                        log_emit(
                            &ah1,
                            &lb1,
                            "info",
                            format!("new message from {}", msg.from_addr),
                        );
                    }
                },
                Arc::new(move |level: &str, msg: String| {
                    log_emit(&ah2, &lb2, level, msg);
                }),
            )
            .await;
    });

    Ok(())
}

#[tauri::command]
pub async fn stop_polling(state: State<'_, AppState>) -> Result<(), String> {
    state
        .core
        .poller_running
        .store(false, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub async fn auto_start_plugins(state: State<'_, AppState>) -> Result<(), String> {
    state.log("info", "auto_start_plugins: no plugins on mobile".into());
    Ok(())
}

#[tauri::command]
pub async fn get_known_hostnames(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let addrs = state
        .core
        .store
        .get_unique_addresses()
        .await
        .map_err(|e| e.to_string())?;
    let mut hostnames: Vec<String> = addrs
        .iter()
        .filter_map(|a| {
            let idx = a.rfind('@')?;
            Some(a[idx + 1..].to_string())
        })
        .collect();
    hostnames.sort();
    hostnames.dedup();
    Ok(hostnames)
}

#[tauri::command]
pub async fn toggle_hide_conversation(
    state: State<'_, AppState>,
    address: String,
    hidden: bool,
) -> Result<(), String> {
    state
        .core
        .store
        .set_hidden(&address, hidden)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_hidden_addresses(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    state
        .core
        .store
        .get_hidden_addresses()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_conversation(
    state: State<'_, AppState>,
    address: String,
) -> Result<(), String> {
    state
        .core
        .store
        .delete_messages_for_address(&address)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_contact_hashes(
    state: State<'_, AppState>,
) -> Result<Vec<(String, String)>, String> {
    state
        .core
        .store
        .get_contact_hashes()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_logs(
    state: State<'_, AppState>,
    limit: u32,
) -> Result<Vec<LogEntry>, String> {
    let buf = state.log_buffer.lock().unwrap();
    let len = buf.len();
    Ok(buf
        .iter()
        .skip(len.saturating_sub(limit as usize))
        .cloned()
        .collect())
}
