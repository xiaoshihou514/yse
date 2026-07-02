pub mod imap;
pub mod smtp;
pub mod parser;

pub use imap::{ImapPoller, ImapConfig};
pub use smtp::{SmtpSender, SmtpConfig};
pub use parser::parse_incoming;
