use serde::{Deserialize, Serialize};

/// Request from plugin to core
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum PluginRequest {
    /// Plugin asks core to send a message
    #[serde(rename = "send")]
    Send {
        #[serde(rename = "from")]
        from_addr: String,
        #[serde(rename = "to")]
        to_addr: String,
        text: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        meta: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        files: Option<Vec<crate::message::FileAttachment>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        plugin_id: Option<String>,
    },
    /// Plugin logs a message
    #[serde(rename = "log")]
    Log { level: String, msg: String },
}

/// Response from core to plugin (for requests with id)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResponse {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Notification from core to plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum CoreNotification {
    /// A new message arrived
    #[serde(rename = "message")]
    Message {
        #[serde(rename = "from")]
        from_addr: String,
        #[serde(rename = "to")]
        to_addr: String,
        text: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        meta: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        files: Option<Vec<crate::message::FileAttachment>>,
    },
    /// Initial configuration sent once after process start.
    /// Tells the plugin where to persist its state files.
    #[serde(rename = "config")]
    Config {
        state_dir: String,
        /// The plugin's virtual address (name#hash@hostname), so it
        /// can use the correct from-address when sending replies.
        virtual_addr: Option<String>,
        /// The local user's address, so plugins know to whom they
        /// should send welcome/notification messages.
        user_addr: String,
    },
    /// Plugin should shut down
    #[serde(rename = "shutdown")]
    Shutdown,
}
