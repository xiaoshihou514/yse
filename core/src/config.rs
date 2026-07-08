use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMapping {
    /// Virtual address pattern (e.g. "bot@yse.org")
    pub virtual_addr: String,
    /// Plugin id that handles this address
    pub plugin_id: String,
    /// Human-readable display name (local-only, never sent over the wire)
    #[serde(default)]
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YseConfig {
    /// Real email credentials
    pub email_imap_server: String,
    pub email_imap_port: u16,
    pub email_smtp_server: String,
    pub email_smtp_port: u16,
    pub email_username: String,
    pub email_password: String,

    /// Own virtual identity
    pub own_address: String,

    /// Encryption password (derived into ChaCha20-Poly1305 key via Argon2id)
    pub crypto_password: String,

    /// Mapping from virtual address → plugin id
    pub plugin_mappings: Vec<PluginMapping>,

    /// Data directory for SQLite DB (runtime-only, not persisted)
    #[serde(skip)]
    pub data_dir: PathBuf,
}

impl Default for YseConfig {
    fn default() -> Self {
        Self {
            email_imap_server: "imap.qq.com".into(),
            email_imap_port: 993,
            email_smtp_server: "smtp.qq.com".into(),
            email_smtp_port: 465,
            email_username: String::new(),
            email_password: String::new(),
            own_address: "me".into(),
            crypto_password: String::new(),
            plugin_mappings: Vec::new(),
            data_dir: PathBuf::from("."),
        }
    }
}
