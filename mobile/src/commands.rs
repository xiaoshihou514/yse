use std::sync::Arc;
use tauri::{Emitter, State};
use yse_core::{
    app::CoreState,
    config::YseConfig,
    email::imap::{ImapConfig, ImapPoller},
    identity,
    message::Message,
};

pub struct AppState {
    pub core: CoreState,
    pub app_handle: Arc<std::sync::Mutex<Option<tauri::AppHandle>>>,
}

impl AppState {
    pub fn new(db_path: impl AsRef<std::path::Path>) -> Result<Self, String> {
        let core = CoreState::new(db_path, "me")?;
        Ok(Self {
            core,
            app_handle: Arc::new(std::sync::Mutex::new(None)),
        })
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
pub async fn set_local_hostname(
    state: State<'_, AppState>,
    hostname: String,
) -> Result<(), String> {
    state.core.session_registry.set_local_hostname(&hostname);
    Ok(())
}

#[tauri::command]
pub async fn get_messages(state: State<'_, AppState>, limit: u32) -> Result<Vec<Message>, String> {
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

    if let Some(h) = state.app_handle.lock().unwrap().as_ref() {
        let _ = h.emit("new-message", &msg);
    }

    if let Err(e) = state.core.send_encrypted(&msg).await {
        log::error!("{}", e);
    }

    log::info!("sent message to {}", to);
    Ok(())
}

#[tauri::command]
pub async fn start_polling(
    state: State<'_, AppState>,
    _app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let (imap_cfg, key, store, own_addr) = {
        let cfg = state.core.config.read().await;
        let imap = ImapConfig {
            server: cfg.email_imap_server.clone(),
            port: cfg.email_imap_port,
            username: cfg.email_username.clone(),
            password: cfg.email_password.clone(),
        };
        let key = state.core.crypto_key.read().await;
        let key = key.as_ref().copied().ok_or("crypto key not set")?;
        (imap, key, state.core.store.clone(), cfg.own_address.clone())
    };

    let last_uid = store
        .get_config_value("imap_last_uid")
        .await
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok());

    state
        .core
        .poller_running
        .store(true, std::sync::atomic::Ordering::SeqCst);

    let poller_flag = state.core.poller_running.clone();
    let emit_handle = state.app_handle.clone();
    let save_store = store.clone();

    log::info!("IMAP polling starting on mobile");

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

        let sr = yse_core::plugin::session::SessionRegistry::new(&own_addr);
        let pm = yse_core::plugin::process_manager::PluginProcessManager::new();
        let eh = emit_handle.clone();
        poller
            .run_with_log(
                move |raw_email| {
                    let parsed = match yse_core::email::parser::parse_incoming(&raw_email) {
                        Ok(p) => p,
                        Err(e) => {
                            log::warn!("IMAP parse failed: {}", e);
                            return;
                        }
                    };
                    let decrypted = match yse_core::crypto::decrypt(&key, &parsed.encrypted_body) {
                        Ok(d) => d,
                        Err(e) => {
                            log::error!("decrypt failed: {}", e);
                            return;
                        }
                    };

                    let msg = match Message::from_json(&decrypted) {
                        Ok(m) => m,
                        Err(e) => {
                            log::error!("message parse failed: {}", e);
                            return;
                        }
                    };

                    let result = tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(async {
                            let s: &dyn yse_core::store::Storage = &*store;
                            yse_core::imap_ingest::ingest_message(&msg, s, &own_addr, &sr, &pm)
                                .await
                        })
                    });

                    if result.show_in_chat {
                        if let Some(h) = eh.lock().unwrap().as_ref() {
                            let _ = h.emit("new-message", &msg);
                        }
                    }
                    log::info!(
                        "new message from {} (show_in_chat={})",
                        msg.from_addr,
                        result.show_in_chat
                    );
                },
                Arc::new(move |level: &str, msg: String| match level {
                    "warn" => log::warn!("{}", msg),
                    "error" => log::error!("{}", msg),
                    "debug" => log::debug!("{}", msg),
                    _ => log::info!("{}", msg),
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
pub async fn auto_start_plugins(_state: State<'_, AppState>) -> Result<(), String> {
    log::info!("auto_start_plugins: no plugins on mobile");
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
        .filter_map(|a| yse_core::identity::extract_hostname(a).map(|s| s.to_string()))
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
