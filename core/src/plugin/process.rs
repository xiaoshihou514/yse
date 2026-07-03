use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, Command as TokioCommand};
use tokio::sync::Mutex;
use tracing::{info, warn};

use super::protocol::*;

/// Callback type for handling plugin → core requests (send, log, etc.)
pub type PluginRequestHandler = Arc<dyn Fn(PluginRequest) + Send + Sync>;

pub struct ManagedPlugin {
    pub id: String,
    process: Option<tokio::process::Child>,
    stdin: Option<Arc<Mutex<ChildStdin>>>,
    #[allow(dead_code)]
    next_id: Arc<AtomicU64>,
}

impl ManagedPlugin {
    pub fn spawn(
        id: String,
        exec_path: &str,
        args: &[String],
        handler: Option<PluginRequestHandler>,
    ) -> Result<Self, String> {
        let mut cmd = TokioCommand::new(exec_path);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let mut child = cmd.spawn().map_err(|e| format!("spawn plugin failed: {}", e))?;

        let stdin = child
            .stdin
            .take()
            .map(|s| Arc::new(Mutex::new(s)))
            .ok_or("failed to open plugin stdin")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("failed to open plugin stdout")?;

        let plugin = ManagedPlugin {
            id: id.clone(),
            process: Some(child),
            stdin: Some(stdin),
            next_id: Arc::new(AtomicU64::new(1)),
        };

        // Spawn stdout reader task that routes plugin requests
        let id_clone = id.clone();
        let handler_id = id.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            let my_id = id_clone;
            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }
                let val: serde_json::Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(e) => {
                        warn!(plugin = %my_id, "invalid JSON: {} | {}", e, line);
                        continue;
                    }
                };
                let method = match val["method"].as_str() {
                    Some(m) => m.to_string(),
                    None => continue,
                };
                let params = val.get("params").cloned();
                let handler = handler.clone();
                let pid = handler_id.clone();
                tokio::spawn(async move {
                    match method.as_str() {
                        "send" => {
                            if let Some(p) = params {
                                if let Some(to) = p["to"].as_str() {
                                    let from = p["from"].as_str().unwrap_or("plugin");
                                    if let Some(h) = handler {
                                        h(PluginRequest::Send {
                                            from_addr: from.to_string(),
                                            to_addr: to.to_string(),
                                            text: p["text"].as_str().map(String::from),
                                            meta: p.get("meta").cloned(),
                                            files: None,
                                        });
                                    }
                                }
                            }
                        }
                        "log" => {
                            if let Some(p) = params {
                                let level = p["level"].as_str().unwrap_or("info");
                                let msg = p["msg"].as_str().unwrap_or("");
                                info!(plugin = %pid, "[{}] {}", level, msg);
                                if let Some(h) = handler {
                                    h(PluginRequest::Log {
                                        level: level.to_string(),
                                        msg: msg.to_string(),
                                    });
                                }
                            }
                        }
                        _ => {
                            info!(plugin = %pid, "unhandled request: {}", method);
                        }
                    }
                });
            }
            warn!(plugin = %my_id, "stdout closed");
        });

        Ok(plugin)
    }

    pub async fn send_notification(&self, notif: &CoreNotification) -> Result<(), String> {
        let json = serde_json::to_string(notif).map_err(|e| e.to_string())?;
        let mut line = json;
        line.push('\n');
        if let Some(stdin) = &self.stdin {
            let mut handle = stdin.lock().await;
            handle
                .write_all(line.as_bytes())
                .await
                .map_err(|e| format!("write to plugin stdin failed: {}", e))?;
            handle.flush().await.map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub async fn send_message_notification(
        &self,
        from: &str,
        to: &str,
        text: Option<&str>,
        meta: Option<&serde_json::Value>,
        files: Option<&Vec<crate::message::FileAttachment>>,
    ) -> Result<(), String> {
        let notif = CoreNotification::Message {
            from_addr: from.into(),
            to_addr: to.into(),
            text: text.map(String::from),
            meta: meta.cloned(),
            files: files.cloned(),
        };
        self.send_notification(&notif).await
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        self.send_notification(&CoreNotification::Shutdown).await
    }

    pub fn kill(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.start_kill();
        }
    }
}

impl Drop for ManagedPlugin {
    fn drop(&mut self) {
        self.kill();
    }
}

pub struct PluginManager {
    plugins: Arc<Mutex<HashMap<String, ManagedPlugin>>>,
    request_handler: std::sync::Mutex<Option<PluginRequestHandler>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(Mutex::new(HashMap::new())),
            request_handler: std::sync::Mutex::new(None),
        }
    }

    pub fn set_request_handler(&self, handler: PluginRequestHandler) {
        *self.request_handler.lock().unwrap() = Some(handler);
    }

    pub fn get_request_handler(&self) -> Option<PluginRequestHandler> {
        self.request_handler.lock().unwrap().clone()
    }

    pub async fn start_plugin(
        &self,
        id: &str,
        exec_path: &str,
        args: &[String],
    ) -> Result<(), String> {
        let mut map = self.plugins.lock().await;
        if map.contains_key(id) {
            return Err(format!("plugin {} already running", id));
        }
        let handler = self.get_request_handler();
        let plugin = ManagedPlugin::spawn(id.into(), exec_path, args, handler)?;
        map.insert(id.into(), plugin);
        info!("plugin started: {}", id);
        Ok(())
    }

    pub async fn stop_plugin(&self, id: &str) -> Result<(), String> {
        let mut map = self.plugins.lock().await;
        if let Some(mut plugin) = map.remove(id) {
            plugin.shutdown().await.ok();
            plugin.kill();
            info!("plugin stopped: {}", id);
            Ok(())
        } else {
            Err(format!("plugin {} not found", id))
        }
    }

    pub async fn dispatch_message(
        &self,
        to_addr: &str,
        from_addr: &str,
        text: Option<&str>,
        meta: Option<&serde_json::Value>,
        files: Option<&Vec<crate::message::FileAttachment>>,
        mapping: &[(String, String)],
    ) {
        let map = self.plugins.lock().await;
        for (vaddr, pid) in mapping {
            if vaddr == to_addr {
                if let Some(plugin) = map.get(pid.as_str()) {
                    let _ = plugin
                        .send_message_notification(from_addr, to_addr, text, meta, files)
                        .await;
                }
            }
        }
    }

    pub async fn list_running(&self) -> Vec<String> {
        let map = self.plugins.lock().await;
        map.keys().cloned().collect()
    }
}
