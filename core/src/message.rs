use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MessageError {
    #[error("serialization failed: {0}")]
    Serialize(String),
    #[error("deserialization failed: {0}")]
    Deserialize(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAttachment {
    pub name: String,
    pub mime: String,
    pub size: u64,
    /// Maps to the MIME attachment filename (e.g. "f1.bin")
    pub enc_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub protocol: String,
    #[serde(rename = "from")]
    pub from_addr: String,
    #[serde(rename = "to")]
    pub to_addr: String,
    pub timestamp: u64,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<FileAttachment>>,
    /// Arbitrary metadata set by plugins, core passes through
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
}

impl Message {
    pub fn new(from: String, to: String, text: Option<String>) -> Self {
        Self {
            protocol: "yse.v1".into(),
            from_addr: from,
            to_addr: to,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            id: uuid::Uuid::new_v4().to_string(),
            text,
            files: None,
            meta: None,
        }
    }

    pub fn with_meta(mut self, meta: serde_json::Value) -> Self {
        self.meta = Some(meta);
        self
    }

    pub fn with_files(mut self, files: Vec<FileAttachment>) -> Self {
        self.files = Some(files);
        self
    }

    pub fn to_json(&self) -> Result<Vec<u8>, MessageError> {
        serde_json::to_vec(self).map_err(|e| MessageError::Serialize(e.to_string()))
    }

    pub fn from_json(data: &[u8]) -> Result<Self, MessageError> {
        serde_json::from_slice(data).map_err(|e| MessageError::Deserialize(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_deserialize() {
        let msg = Message::new(
            "alice@yse.org".into(),
            "bob@yse.org".into(),
            Some("你好".into()),
        );
        let json = msg.to_json().unwrap();
        let parsed = Message::from_json(&json).unwrap();
        assert_eq!(parsed.from_addr, "alice@yse.org");
        assert_eq!(parsed.to_addr, "bob@yse.org");
        assert_eq!(parsed.text, Some("你好".into()));
        assert_eq!(parsed.protocol, "yse.v1");
    }

    #[test]
    fn test_with_files() {
        let mut msg = Message::new(
            "alice@yse.org".into(),
            "bob@yse.org".into(),
            Some("看文件".into()),
        );
        msg.files = Some(vec![FileAttachment {
            name: "方案.pdf".into(),
            mime: "application/pdf".into(),
            size: 1048576,
            enc_name: "f1.bin".into(),
        }]);
        let json = msg.to_json().unwrap();
        let parsed = Message::from_json(&json).unwrap();
        assert_eq!(parsed.files.unwrap().len(), 1);
    }
}
