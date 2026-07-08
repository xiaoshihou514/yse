pub mod app;
pub mod config;
pub mod crypto;
pub mod disguise;
pub mod email;
pub mod event;
pub mod identity;
pub mod imap_ingest;
pub mod logging;
pub mod message;
pub mod plugin;
pub mod router;
pub mod store;

pub use config::{PluginMapping, YseConfig};
pub use crypto::{decrypt, derive_key, encrypt, CryptoError};
pub use disguise::{disguise, DisguisedSender};
pub use message::{FileAttachment, Message, MessageError};
