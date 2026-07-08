use crate::identity;
use crate::message::Message;
use crate::plugin::session::RouteResult;
use crate::store::Storage;
use tracing::{info, warn};

/// Result of processing an incoming IMAP message.
pub struct IngestResult {
    /// Whether the message should be shown in the user's chat.
    pub show_in_chat: bool,
    /// Route result from plugin routing (desktop only, None on mobile).
    pub route_result: Option<RouteResult>,
}

/// Common classification: is this message for our own address?
fn classify(msg: &Message, own_addr: &str) -> (/*for_self*/ bool, /*for_user*/ bool) {
    let own_name = own_addr.split('@').next().unwrap_or(own_addr);
    let to_name = identity::parse_address(&msg.to_addr)
        .map(|(n, _, _)| n).unwrap_or("");
    let from_name = identity::parse_address(&msg.from_addr)
        .map(|(n, _, _)| n).unwrap_or("");

    let for_self = to_name == own_name
        || msg.to_addr == own_name
        || msg.to_addr == own_addr;

    let for_user = for_self
        || from_name == own_name
        || msg.from_addr == own_name
        || msg.from_addr == own_addr;

    (for_self, for_user)
}

/// Shared prelude: save, deduplicate, and check if routing is needed.
/// Returns (for_user, should_route).
async fn ingest_core(msg: &Message, store: &dyn Storage, own_addr: &str) -> (bool, bool) {
    let (for_self, for_user) = classify(msg, own_addr);

    let already = store.is_processed(&msg.id).await.unwrap_or(false);
    if let Err(e) = store.save_message(msg).await {
        warn!("imap: save_message failed for {}: {}", msg.id, e);
    }
    if already {
        return (for_user, false);
    }

    if for_self {
        if let Err(e) = store.mark_processed(&msg.id).await {
            warn!("imap: mark_processed failed for {}: {}", msg.id, e);
        }
        info!("imap: self-addressed msg {}, marked processed", msg.id);
        return (for_user, false);
    }

    (for_user, true)
}

/// Process an incoming IMAP message: save, dedup, route to plugin if needed.
/// Uses SessionRegistry for plugin routing (pass dummy values on mobile).
pub async fn ingest_message(
    msg: &Message,
    store: &dyn Storage,
    own_addr: &str,
    sr: &crate::plugin::session::SessionRegistry,
    pm: &crate::plugin::process_manager::PluginProcessManager,
) -> IngestResult {
    let (for_user, should_route) = ingest_core(msg, store, own_addr).await;

    let route_result = if should_route {
        let plugin_configs = store.list_plugins().await.unwrap_or_default();
        let r = sr
            .route(
                &msg.to_addr,
                &msg.from_addr,
                msg.text.as_deref(),
                msg.meta.as_ref(),
                msg.files.as_deref(),
                &plugin_configs,
                pm,
            )
            .await;
        Some(r)
    } else {
        None
    };

    if let Err(e) = store.mark_processed(&msg.id).await {
        warn!("imap: mark_processed failed for {}: {}", msg.id, e);
    }

    IngestResult { show_in_chat: for_user, route_result }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Message;
    use crate::store::{Storage, StoreError, PluginConfig};
    use async_trait::async_trait;
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeStore {
        messages: Mutex<Vec<Message>>,
        processed: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl Storage for FakeStore {
        async fn save_message(&self, msg: &Message) -> Result<(), StoreError> {
            self.messages.lock().unwrap().push(msg.clone());
            Ok(())
        }
        async fn list_messages(&self, _: u32, _: u32) -> Result<Vec<Message>, StoreError> {
            Ok(self.messages.lock().unwrap().clone())
        }
        async fn is_processed(&self, msg_id: &str) -> Result<bool, StoreError> {
            Ok(self.processed.lock().unwrap().contains(&msg_id.to_string()))
        }
        async fn mark_processed(&self, msg_id: &str) -> Result<(), StoreError> {
            self.processed.lock().unwrap().push(msg_id.to_string());
            Ok(())
        }
        async fn list_plugins(&self) -> Result<Vec<PluginConfig>, StoreError> { Ok(vec![]) }
        async fn save_plugin(&self, _: &PluginConfig) -> Result<(), StoreError> { Ok(()) }
        async fn delete_plugin(&self, _: &str) -> Result<(), StoreError> { Ok(()) }
        async fn get_config_value(&self, _: &str) -> Result<Option<String>, StoreError> { Ok(None) }
        async fn set_config_value(&self, _: &str, _: &str) -> Result<(), StoreError> { Ok(()) }
        async fn get_contact_hashes(&self) -> Result<Vec<(String, String)>, StoreError> { Ok(vec![]) }
        async fn save_contact_hash(&self, _: &str, _: &str) -> Result<(), StoreError> { Ok(()) }
        async fn get_unique_addresses(&self) -> Result<Vec<String>, StoreError> { Ok(vec![]) }
        async fn set_hidden(&self, _: &str, _: bool) -> Result<(), StoreError> { Ok(()) }
        async fn get_hidden_addresses(&self) -> Result<Vec<String>, StoreError> { Ok(vec![]) }
        async fn delete_messages_for_address(&self, _: &str) -> Result<(), StoreError> { Ok(()) }
    }

    fn make_msg(from: &str, to: &str) -> Message {
        Message::new(from.into(), to.into(), Some("test".into()))
    }

    fn block<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Runtime::new().unwrap().block_on(f)
    }

    #[test]
    fn classify_self_addressed() {
        let m = make_msg("me#abc@myhost", "me#def@myhost");
        let (fs, fu) = classify(&m, "me");
        assert!(fs);
        assert!(fu);
    }

    #[test]
    fn classify_from_other_to_self() {
        let m = make_msg("echo#abc@myhost", "me#def@myhost");
        let (fs, fu) = classify(&m, "me");
        assert!(fs);
        assert!(fu);
    }

    #[test]
    fn classify_from_self_to_other() {
        let m = make_msg("me#abc@myhost", "echo#def@myhost");
        let (fs, fu) = classify(&m, "me");
        assert!(!fs);
        assert!(fu);
    }

    #[test]
    fn classify_unrelated() {
        let m = make_msg("alice#abc@other", "bob#def@other");
        let (fs, fu) = classify(&m, "me");
        assert!(!fs);
        assert!(!fu);
    }

    #[test]
    fn ingest_core_new_msg_routes() {
        let store = FakeStore::default();
        let msg = make_msg("me#abc@myhost", "echo#def@myhost");
        let (fu, should_route) = block(ingest_core(&msg, &store, "me"));
        assert!(fu);
        assert!(should_route);
        assert_eq!(store.messages.lock().unwrap().len(), 1);
    }

    #[test]
    fn ingest_core_self_addressed_no_route() {
        let store = FakeStore::default();
        let msg = make_msg("me#abc@myhost", "me#def@myhost");
        let (fu, should_route) = block(ingest_core(&msg, &store, "me"));
        assert!(fu);
        assert!(!should_route);
        assert!(block(store.is_processed(&msg.id)).unwrap());
    }

    #[test]
    fn ingest_core_already_processed_skips() {
        let store = FakeStore::default();
        let msg = make_msg("echo#abc@myhost", "me#def@myhost");
        block(store.mark_processed(&msg.id)).unwrap();
        let (_fu, should_route) = block(ingest_core(&msg, &store, "me"));
        assert!(!should_route);
    }

    #[test]
    fn ingest_core_unrelated_not_for_user() {
        let store = FakeStore::default();
        let msg = make_msg("alice#abc@other", "bob#def@other");
        let (fu, _should_route) = block(ingest_core(&msg, &store, "me"));
        assert!(!fu);
    }
}
