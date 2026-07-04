use crate::message::Message;
use async_trait::async_trait;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("database error: {0}")]
    Db(String),
    #[error("not found")]
    NotFound,
}

#[async_trait]
pub trait Storage: Send + Sync {
    /// Save a message (dedup by id internally).
    async fn save_message(&self, msg: &Message) -> Result<(), StoreError>;
    /// List messages matching filter.
    async fn list_messages(&self, limit: u32, offset: u32) -> Result<Vec<Message>, StoreError>;
    /// Check if a message id was already processed.
    async fn is_processed(&self, msg_id: &str) -> Result<bool, StoreError>;
    /// Mark message as processed.
    async fn mark_processed(&self, msg_id: &str) -> Result<(), StoreError>;

    // Plugin config
    async fn list_plugins(&self) -> Result<Vec<PluginConfig>, StoreError>;
    async fn save_plugin(&self, plugin: &PluginConfig) -> Result<(), StoreError>;
    async fn delete_plugin(&self, id: &str) -> Result<(), StoreError>;

    // App config key-value
    async fn get_config_value(&self, key: &str) -> Result<Option<String>, StoreError>;
    async fn set_config_value(&self, key: &str, value: &str) -> Result<(), StoreError>;
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PluginConfig {
    pub id: String,
    pub name: String,
    pub exec_path: String,
    pub args: Vec<String>,
    pub enabled: bool,
}

pub mod sqlite;
