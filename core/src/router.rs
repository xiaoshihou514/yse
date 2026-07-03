use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::sync::Mutex;
use tracing::info;

use crate::message::Message;
use crate::plugin::process::PluginManager;

pub struct Router {
    pending: Arc<Mutex<BTreeMap<u64, Vec<Message>>>>,
    last_delivered: Arc<Mutex<HashMap<String, u64>>>,
    plugin_manager: Arc<PluginManager>,
    mappings: Arc<Mutex<Vec<(String, String)>>>,
    on_self_message: Arc<StdMutex<Option<Box<dyn Fn(Message) + Send>>>>,
}

impl Router {
    pub fn new(plugin_manager: Arc<PluginManager>) -> Self {
        Self {
            pending: Arc::new(Mutex::new(BTreeMap::new())),
            last_delivered: Arc::new(Mutex::new(HashMap::new())),
            plugin_manager,
            mappings: Arc::new(Mutex::new(Vec::new())),
            on_self_message: Arc::new(StdMutex::new(None)),
        }
    }

    pub fn set_on_self_message<F>(&self, f: F)
    where
        F: Fn(Message) + Send + 'static,
    {
        let mut cb = self.on_self_message.lock().unwrap();
        *cb = Some(Box::new(f));
    }

    pub async fn update_mappings(&self, mappings: Vec<(String, String)>) {
        let mut m = self.mappings.lock().await;
        *m = mappings;
    }

    pub async fn ingest(&self, msg: Message, own_address: &str) {
        {
            let delivered = self.last_delivered.lock().await;
            if let Some(&last_ts) = delivered.get(&msg.from_addr) {
                if msg.timestamp <= last_ts {
                    info!("duplicate message dropped: {}", msg.id);
                    return;
                }
            }
        }

        if msg.to_addr == own_address {
            if let Some(cb) = self.on_self_message.lock().unwrap().as_ref() {
                cb(msg.clone());
            }
        }

        let mappings = self.mappings.lock().await;
        let _ = self.plugin_manager
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

    pub async fn flush_pending(&self, from_addr: &str) {
        let mut pending = self.pending.lock().await;
        let mut delivered = self.last_delivered.lock().await;
        let current_last = delivered.get(from_addr).copied().unwrap_or(0);

        let mut to_deliver = Vec::new();
        let mut to_remove = Vec::new();

        for (&ts, msgs) in pending.range(current_last + 1..) {
            for msg in msgs {
                if msg.from_addr == from_addr {
                    to_deliver.push(msg.clone());
                    to_remove.push(ts);
                }
            }
        }

        for ts in to_remove {
            pending.remove(&ts);
        }

        if let Some(max_ts) = to_deliver.iter().map(|m| m.timestamp).max() {
            delivered.insert(from_addr.into(), max_ts);
        }

        for msg in to_deliver {
            let mappings = self.mappings.lock().await;
            let _ = self.plugin_manager
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
    }
}
