use tokio::sync::broadcast;
use crate::message::Message;

#[derive(Debug, Clone)]
pub enum CoreEvent {
    /// A new message was received and processed
    MessageReceived(Message),
    /// Plugin state changed
    PluginStateChanged { id: String, running: bool },
    /// Error occurred
    Error { source: String, detail: String },
    /// Log entry
    Log { level: String, message: String },
}

pub type EventSender = broadcast::Sender<CoreEvent>;
pub type EventReceiver = broadcast::Receiver<CoreEvent>;

pub fn event_channel() -> (EventSender, EventReceiver) {
    broadcast::channel(256)
}
