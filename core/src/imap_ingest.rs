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
    if let Err(e) = store.save_message(&msg).await {
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

/// Desktop version: includes plugin routing via SessionRegistry.
#[cfg(feature = "plugin-routing")]
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
                msg.files.as_ref(),
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

/// Mobile version: no plugin routing.
#[cfg(not(feature = "plugin-routing"))]
pub async fn ingest_message(
    msg: &Message,
    store: &dyn Storage,
    own_addr: &str,
) -> IngestResult {
    let (for_user, _should_route) = ingest_core(msg, store, own_addr).await;

    if let Err(e) = store.mark_processed(&msg.id).await {
        warn!("imap: mark_processed failed for {}: {}", msg.id, e);
    }

    IngestResult { show_in_chat: for_user, route_result: None }
}
