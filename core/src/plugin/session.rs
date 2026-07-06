use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

use super::process_manager::PluginProcessManager;
use super::protocol::CoreNotification;
use crate::identity;
use crate::message::FileAttachment;
use crate::store::PluginConfig;

#[derive(Debug, Clone, serde::Serialize)]
pub struct Session {
    pub hash: String,
    pub plugin_id: String,
    pub name: String,
}

pub struct SessionRegistry {
    sessions: Arc<Mutex<HashMap<String, Session>>>,
    contact_hashes: std::sync::Mutex<HashMap<String, String>>,
    local_name: std::sync::Mutex<String>,
}

impl SessionRegistry {
    pub fn new(local_name: &str) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            contact_hashes: std::sync::Mutex::new(HashMap::new()),
            local_name: std::sync::Mutex::new(local_name.to_string()),
        }
    }

    pub fn set_local_name(&self, name: &str) {
        *self.local_name.lock().unwrap() = name.to_string();
    }

    pub fn set_contact_hashes(&self, hashes: HashMap<String, String>) {
        *self.contact_hashes.lock().unwrap() = hashes;
    }

    pub fn get_contact_hashes(&self) -> HashMap<String, String> {
        self.contact_hashes.lock().unwrap().clone()
    }

    pub fn get_or_create_sender_hash(&self, recipient: &str) -> String {
        let mut map = self.contact_hashes.lock().unwrap();
        if let Some(h) = map.get(recipient) {
            return h.clone();
        }
        let hash = identity::generate_hash();
        map.insert(recipient.to_string(), hash.clone());
        hash
    }

    pub fn format_sender_address(&self, recipient: &str) -> String {
        let hash = self.get_or_create_sender_hash(recipient);
        let name = self.local_name.lock().unwrap().clone();
        let hostname = identity::local_hostname();
        identity::format_address(&name, &hash, &hostname)
    }

    /// Route a message to the right plugin, starting one if needed.
    pub async fn route(
        &self,
        to_addr: &str,
        from_addr: &str,
        text: Option<&str>,
        meta: Option<&serde_json::Value>,
        files: Option<&Vec<FileAttachment>>,
        plugin_configs: &[PluginConfig],
        process_manager: &PluginProcessManager,
    ) -> bool {
        let (name, hash, hostname) = match identity::parse_address(to_addr) {
            Some(p) => p,
            None => {
                info!("route: cannot parse address: {}", to_addr);
                return false;
            }
        };

        let our_host = identity::local_hostname();
        if hostname != our_host {
            info!("route: message for another host: {} (we are {})", hostname, our_host);
            return false;
        }

        let plugin_id = self.resolve_plugin(name, hash, plugin_configs, process_manager).await;

        match plugin_id {
            Some(pid) => {
                let notif = CoreNotification::Message {
                    from_addr: from_addr.to_string(),
                    to_addr: to_addr.to_string(),
                    text: text.map(String::from),
                    meta: meta.cloned(),
                    files: files.cloned(),
                };
                self.dispatch_to_plugin(&pid, &notif, process_manager).await;
                true
            }
            None => {
                warn!("route: no plugin found for name={} hash={}", name, hash);
                false
            }
        }
    }

    async fn resolve_plugin(
        &self,
        name: &str,
        hash: &str,
        plugin_configs: &[PluginConfig],
        process_manager: &PluginProcessManager,
    ) -> Option<String> {
        {
            let sessions = self.sessions.lock().await;
            if let Some(session) = sessions.get(hash) {
                if process_manager.is_running(&session.plugin_id).await {
                    return Some(session.plugin_id.clone());
                }
            }
        }

        let config = plugin_configs.iter().find(|p| p.id == name || p.name == name)?.clone();
        let plugin_id = config.id.clone();

        if let Err(e) = process_manager
            .start(&plugin_id, &config.name, &config.exec_path, &config.args)
            .await
        {
            warn!("resolve_plugin: start failed for {}: {}", plugin_id, e);
            return None;
        }

        let mut sessions = self.sessions.lock().await;
        sessions.insert(
            hash.to_string(),
            Session {
                hash: hash.to_string(),
                plugin_id: plugin_id.clone(),
                name: name.to_string(),
            },
        );

        info!("session registered: name={} hash={} plugin_id={}", name, hash, plugin_id);
        Some(plugin_id)
    }

    async fn dispatch_to_plugin(
        &self,
        plugin_id: &str,
        notif: &CoreNotification,
        process_manager: &PluginProcessManager,
    ) {
        if let Err(e) = process_manager.send_notification(plugin_id, notif).await {
            warn!("dispatch_to_plugin {} failed: {}", plugin_id, e);
        }
    }

    pub async fn list_sessions(&self) -> Vec<Session> {
        let sessions = self.sessions.lock().await;
        sessions.values().cloned().collect()
    }

    pub async fn remove_session(&self, hash: &str) {
        let mut sessions = self.sessions.lock().await;
        sessions.remove(hash);
    }
}
