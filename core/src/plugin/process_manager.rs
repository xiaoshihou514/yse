use log::{info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

use super::process::ManagedPlugin;
use super::protocol::{CoreNotification, PluginRequest};

pub type PluginRequestHandler = Arc<dyn Fn(PluginRequest) + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum ProcessState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Crashed(String),
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProcessInfo {
    pub id: String,
    pub name: String,
    pub exec_path: String,
    pub args: Vec<String>,
    pub state: ProcessState,
    pub start_time: Option<u64>,
    pub restart_count: u32,
    pub last_exit: Option<String>,
}

struct ProcessEntry {
    id: String,
    name: String,
    exec_path: String,
    args: Vec<String>,
    state: ProcessState,
    plugin: Option<ManagedPlugin>,
    start_time: Option<Instant>,
    restart_count: u32,
    last_exit: Option<String>,
}

#[derive(Clone)]
pub struct PluginProcessManager {
    processes: Arc<Mutex<HashMap<String, ProcessEntry>>>,
    request_handler: Arc<std::sync::Mutex<Option<PluginRequestHandler>>>,
    app_data_dir: Arc<std::sync::Mutex<Option<String>>>,
}

impl Default for PluginProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginProcessManager {
    pub fn new() -> Self {
        Self {
            processes: Arc::new(Mutex::new(HashMap::new())),
            request_handler: Arc::new(std::sync::Mutex::new(None)),
            app_data_dir: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    pub fn set_app_data_dir(&self, dir: String) {
        std::fs::create_dir_all(&dir).ok();
        *self.app_data_dir.lock().unwrap() = Some(dir);
    }

    pub fn set_request_handler(&self, handler: PluginRequestHandler) {
        *self.request_handler.lock().unwrap() = Some(handler);
    }

    fn get_request_handler(&self) -> Option<PluginRequestHandler> {
        self.request_handler.lock().unwrap().clone()
    }

    /// Shared crash handler — used by both start() and restart().
    fn make_on_exit(
        processes: Arc<Mutex<HashMap<String, ProcessEntry>>>,
        id: String,
    ) -> Box<dyn FnOnce(String) + Send> {
        Box::new(move |_| {
            let procs = processes;
            tokio::spawn(async move {
                let mut map = procs.lock().await;
                if let Some(entry) = map.get_mut(&id) {
                    let msg = if entry.restart_count < 3 {
                        format!("crashed (restart {}/3 possible)", entry.restart_count)
                    } else {
                        "crashed (max restarts)".to_string()
                    };
                    warn!("process {}: {}", id, msg);
                    entry.state = ProcessState::Crashed(msg.clone());
                    entry.last_exit = Some(msg);
                }
            });
        })
    }

    pub async fn start(
        &self,
        id: &str,
        name: &str,
        exec_path: &str,
        args: &[String],
    ) -> Result<(), String> {
        let mut map = self.processes.lock().await;
        if let Some(entry) = map.get(id) {
            if entry.state == ProcessState::Running || entry.state == ProcessState::Starting {
                return Err(format!(
                    "plugin {} is already active ({:?})",
                    id, entry.state
                ));
            }
        }

        let handler = self.get_request_handler();
        let on_exit = Some(Self::make_on_exit(self.processes.clone(), id.to_string()));
        let state_dir = self
            .app_data_dir
            .lock()
            .unwrap()
            .clone()
            .map(|d| format!("{}/plugins/{}", d, id))
            .unwrap_or_else(|| format!("./plugins/{}", id));

        match ManagedPlugin::spawn_with_exit_handler(id.into(), exec_path, args, &state_dir, handler, on_exit) {
            Ok(plugin) => {
                map.insert(
                    id.into(),
                    ProcessEntry {
                        id: id.into(),
                        name: name.into(),
                        exec_path: exec_path.into(),
                        args: args.to_vec(),
                        state: ProcessState::Running,
                        plugin: Some(plugin),
                        start_time: Some(Instant::now()),
                        restart_count: 0,
                        last_exit: None,
                    },
                );
                info!("process started: {} ({})", name, id);
                Ok(())
            }
            Err(e) => {
                map.remove(id);
                Err(e)
            }
        }
    }

    pub async fn stop(&self, id: &str) -> Result<(), String> {
        let mut map = self.processes.lock().await;
        let entry = map
            .get_mut(id)
            .ok_or_else(|| format!("plugin {} not found", id))?;

        if entry.state != ProcessState::Running {
            return Err(format!("plugin {} is not running ({:?})", id, entry.state));
        }

        entry.state = ProcessState::Stopping;

        if let Some(ref plugin) = entry.plugin {
            let _ = plugin.shutdown().await;
        }

        if let Some(mut plugin) = entry.plugin.take() {
            plugin.kill();
        }

        entry.state = ProcessState::Stopped;
        entry.start_time = None;
        info!("process stopped: {} ({})", entry.name, id);
        Ok(())
    }

    pub async fn restart(&self, id: &str) -> Result<(), String> {
        let (name, exec_path, args) = {
            let map = self.processes.lock().await;
            let entry = map
                .get(id)
                .ok_or_else(|| format!("plugin {} not found", id))?;
            (
                entry.name.clone(),
                entry.exec_path.clone(),
                entry.args.clone(),
            )
        };

        let _ = self.stop(id).await;

        let mut map = self.processes.lock().await;
        let restart_count = map.get(id).map(|e| e.restart_count + 1).unwrap_or(0);
        let handler = self.get_request_handler();
        let on_exit = Some(Self::make_on_exit(self.processes.clone(), id.to_string()));
        let state_dir = self
            .app_data_dir
            .lock()
            .unwrap()
            .clone()
            .map(|d| format!("{}/plugins/{}", d, id))
            .unwrap_or_else(|| format!("./plugins/{}", id));

        match ManagedPlugin::spawn_with_exit_handler(id.into(), &exec_path, &args, &state_dir, handler, on_exit)
        {
            Ok(plugin) => {
                map.insert(
                    id.into(),
                    ProcessEntry {
                        id: id.into(),
                        name,
                        exec_path,
                        args,
                        state: ProcessState::Running,
                        plugin: Some(plugin),
                        start_time: Some(Instant::now()),
                        restart_count,
                        last_exit: None,
                    },
                );
                info!("process restarted: {} (attempt {})", id, restart_count);
                Ok(())
            }
            Err(e) => {
                map.remove(id);
                Err(e)
            }
        }
    }

    pub async fn get_info(&self, id: &str) -> Option<ProcessInfo> {
        let map = self.processes.lock().await;
        map.get(id).map(|e| ProcessInfo {
            id: e.id.clone(),
            name: e.name.clone(),
            exec_path: e.exec_path.clone(),
            args: e.args.clone(),
            state: e.state.clone(),
            start_time: e.start_time.map(|i| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64
                    - i.elapsed().as_millis() as u64
            }),
            restart_count: e.restart_count,
            last_exit: e.last_exit.clone(),
        })
    }

    pub async fn list_all(&self) -> Vec<ProcessInfo> {
        let map = self.processes.lock().await;
        map.values()
            .map(|e| ProcessInfo {
                id: e.id.clone(),
                name: e.name.clone(),
                exec_path: e.exec_path.clone(),
                args: e.args.clone(),
                state: e.state.clone(),
                start_time: e.start_time.map(|i| {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64
                        - i.elapsed().as_millis() as u64
                }),
                restart_count: e.restart_count,
                last_exit: e.last_exit.clone(),
            })
            .collect()
    }

    pub async fn is_running(&self, id: &str) -> bool {
        let map = self.processes.lock().await;
        map.get(id)
            .map(|e| e.state == ProcessState::Running)
            .unwrap_or(false)
    }

    pub async fn running_ids(&self) -> Vec<String> {
        let map = self.processes.lock().await;
        map.values()
            .filter(|e| e.state == ProcessState::Running)
            .map(|e| e.id.clone())
            .collect()
    }

    pub async fn running_count(&self) -> usize {
        let map = self.processes.lock().await;
        map.values()
            .filter(|e| e.state == ProcessState::Running)
            .count()
    }

    pub async fn get_logs(&self, id: &str) -> Option<Vec<String>> {
        let buf = {
            let map = self.processes.lock().await;
            map.get(id)
                .and_then(|e| e.plugin.as_ref())
                .map(|p| p.output.clone())?
        };
        let result = buf.lock().await.iter().cloned().collect();
        Some(result)
    }

    pub async fn send_notification(
        &self,
        plugin_id: &str,
        notif: &CoreNotification,
    ) -> Result<(), String> {
        let map = self.processes.lock().await;
        match map.get(plugin_id) {
            Some(entry) => {
                if let Some(ref plugin) = entry.plugin {
                    plugin.send_notification(notif).await
                } else {
                    Err(format!("plugin {} has no process handle", plugin_id))
                }
            }
            None => Err(format!("plugin {} not found in process manager", plugin_id)),
        }
    }
}
