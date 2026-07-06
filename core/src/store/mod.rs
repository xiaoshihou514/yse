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
    async fn save_message(&self, msg: &Message) -> Result<(), StoreError>;
    async fn list_messages(&self, limit: u32, offset: u32) -> Result<Vec<Message>, StoreError>;
    async fn is_processed(&self, msg_id: &str) -> Result<bool, StoreError>;
    async fn mark_processed(&self, msg_id: &str) -> Result<(), StoreError>;

    async fn list_plugins(&self) -> Result<Vec<PluginConfig>, StoreError>;
    async fn save_plugin(&self, plugin: &PluginConfig) -> Result<(), StoreError>;
    async fn delete_plugin(&self, id: &str) -> Result<(), StoreError>;

    async fn get_config_value(&self, key: &str) -> Result<Option<String>, StoreError>;
    async fn set_config_value(&self, key: &str, value: &str) -> Result<(), StoreError>;

    // Contact hashes (persistent per-recipient sender hash)
    async fn save_contact_hash(&self, recipient: &str, local_hash: &str) -> Result<(), StoreError>;
    async fn get_contact_hashes(&self) -> Result<Vec<(String, String)>, StoreError>;

    // Hidden conversations
    async fn set_hidden(&self, addr: &str, hidden: bool) -> Result<(), StoreError>;
    async fn get_hidden_addresses(&self) -> Result<Vec<String>, StoreError>;

    // Get all unique addresses from message history
    async fn get_unique_addresses(&self) -> Result<Vec<String>, StoreError>;
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
