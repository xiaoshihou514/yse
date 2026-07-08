use serde::Serialize;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub level: String,
    pub message: String,
    pub timestamp: u64,
}

impl LogEntry {
    pub fn new(level: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            level: level.into(),
            message: message.into(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        }
    }
}

pub type LogBuffer = Arc<Mutex<Vec<LogEntry>>>;

pub fn log_buffer_push(buf: &LogBuffer, entry: LogEntry) {
    let mut inner = buf.lock().unwrap();
    inner.push(entry);
    if inner.len() > 1000 {
        let keep = inner.len() - 500;
        inner.drain(0..keep);
    }
}

pub fn log_buffer_new() -> LogBuffer {
    Arc::new(Mutex::new(Vec::new()))
}
