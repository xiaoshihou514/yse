use async_trait::async_trait;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;
use tracing::debug;

use crate::message::Message;

use super::{PluginConfig, Storage, StoreError};

pub struct SqliteStorage {
    conn: Mutex<Connection>,
}

impl SqliteStorage {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let conn = Connection::open(path).map_err(|e| StoreError::Db(e.to_string()))?;
        let storage = Self {
            conn: Mutex::new(conn),
        };
        storage.migrate()?;
        Ok(storage)
    }

    fn migrate(&self) -> Result<(), StoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StoreError::Db(e.to_string()))?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                from_addr TEXT NOT NULL,
                to_addr TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                protocol TEXT NOT NULL,
                text TEXT,
                files TEXT,
                meta TEXT,
                processed INTEGER DEFAULT 0,
                created_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS plugins (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                exec_path TEXT NOT NULL,
                args TEXT NOT NULL,
                enabled INTEGER DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS contact_hashes (
                recipient TEXT PRIMARY KEY,
                local_hash TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS hidden_addresses (
                address TEXT PRIMARY KEY
            );

            CREATE TABLE IF NOT EXISTS processed_ids (
                id TEXT PRIMARY KEY
            );
            ",
        )
        .map_err(|e| StoreError::Db(e.to_string()))?;

        // Migrate: copy existing processed marks from messages table to processed_ids
        conn.execute(
            "INSERT OR IGNORE INTO processed_ids (id) SELECT id FROM messages WHERE processed = 1",
            [],
        )
        .map_err(|e| StoreError::Db(e.to_string()))?;

        Ok(())
    }
}

#[async_trait]
impl Storage for SqliteStorage {
    async fn save_message(&self, msg: &Message) -> Result<(), StoreError> {
        debug!(id = %msg.id, from = %msg.from_addr, to = %msg.to_addr, "sql: save_message");
        let conn = self
            .conn
            .lock()
            .map_err(|e| StoreError::Db(e.to_string()))?;
        let files_json = msg
            .files
            .as_ref()
            .map(|f| serde_json::to_string(f).unwrap());
        let meta_json = msg.meta.as_ref().map(|m| m.to_string());
        conn.execute(
            "INSERT OR IGNORE INTO messages (id, from_addr, to_addr, timestamp, protocol, text, files, meta, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?4)",
            rusqlite::params![
                msg.id,
                msg.from_addr,
                msg.to_addr,
                msg.timestamp,
                msg.protocol,
                msg.text,
                files_json,
                meta_json,
            ],
        )
        .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(())
    }

    async fn list_messages(&self, limit: u32, offset: u32) -> Result<Vec<Message>, StoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StoreError::Db(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, from_addr, to_addr, timestamp, protocol, text, files, meta
                 FROM messages ORDER BY timestamp DESC LIMIT ?1 OFFSET ?2",
            )
            .map_err(|e| StoreError::Db(e.to_string()))?;

        let rows = stmt
            .query_map(rusqlite::params![limit, offset], |row| {
                let files_json: Option<String> = row.get(6)?;
                let meta_json: Option<String> = row.get(7)?;
                Ok(Message {
                    protocol: row.get(4)?,
                    from_addr: row.get(1)?,
                    to_addr: row.get(2)?,
                    timestamp: row.get(3)?,
                    id: row.get(0)?,
                    text: row.get(5)?,
                    files: files_json.and_then(|s| serde_json::from_str(&s).ok()),
                    meta: meta_json.and_then(|s| serde_json::from_str(&s).ok()),
                })
            })
            .map_err(|e| StoreError::Db(e.to_string()))?;

        Ok(rows.collect::<Result<Vec<_>, _>>().map_err(|e| StoreError::Db(e.to_string()))?)
    }

    async fn is_processed(&self, msg_id: &str) -> Result<bool, StoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StoreError::Db(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT 1 FROM processed_ids WHERE id = ?1")
            .map_err(|e| StoreError::Db(e.to_string()))?;
        let exists = stmt.exists(rusqlite::params![msg_id])
            .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(exists)
    }

    async fn mark_processed(&self, msg_id: &str) -> Result<(), StoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StoreError::Db(e.to_string()))?;
        conn.execute(
            "INSERT OR IGNORE INTO processed_ids (id) VALUES (?1)",
            rusqlite::params![msg_id],
        )
        .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(())
    }

    async fn list_plugins(&self) -> Result<Vec<PluginConfig>, StoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StoreError::Db(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT id, name, exec_path, args, enabled FROM plugins")
            .map_err(|e| StoreError::Db(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                let args_str: String = row.get(3)?;
                Ok(PluginConfig {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    exec_path: row.get(2)?,
                    args: serde_json::from_str(&args_str).unwrap_or_default(),
                    enabled: row.get::<_, i32>(4)? != 0,
                })
            })
            .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(rows.collect::<Result<Vec<_>, _>>().map_err(|e| StoreError::Db(e.to_string()))?)
    }

    async fn save_plugin(&self, plugin: &PluginConfig) -> Result<(), StoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StoreError::Db(e.to_string()))?;
        let args_str = serde_json::to_string(&plugin.args).unwrap_or_default();
        conn.execute(
            "INSERT OR REPLACE INTO plugins (id, name, exec_path, args, enabled) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![plugin.id, plugin.name, plugin.exec_path, args_str, i32::from(plugin.enabled)],
        )
        .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(())
    }

    async fn delete_plugin(&self, id: &str) -> Result<(), StoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StoreError::Db(e.to_string()))?;
        conn.execute("DELETE FROM plugins WHERE id = ?1", rusqlite::params![id])
            .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(())
    }

    async fn get_config_value(&self, key: &str) -> Result<Option<String>, StoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StoreError::Db(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT value FROM config WHERE key = ?1")
            .map_err(|e| StoreError::Db(e.to_string()))?;
        stmt.query_row(rusqlite::params![key], |row| row.get(0))
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                _ => Err(StoreError::Db(e.to_string())),
            })
    }

    async fn set_config_value(&self, key: &str, value: &str) -> Result<(), StoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StoreError::Db(e.to_string()))?;
        conn.execute(
            "INSERT OR REPLACE INTO config (key, value) VALUES (?1, ?2)",
            rusqlite::params![key, value],
        )
        .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(())
    }

    async fn save_contact_hash(
        &self,
        recipient: &str,
        local_hash: &str,
    ) -> Result<(), StoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StoreError::Db(e.to_string()))?;
        conn.execute(
            "INSERT OR REPLACE INTO contact_hashes (recipient, local_hash) VALUES (?1, ?2)",
            rusqlite::params![recipient, local_hash],
        )
        .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(())
    }

    async fn get_contact_hashes(&self) -> Result<Vec<(String, String)>, StoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StoreError::Db(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT recipient, local_hash FROM contact_hashes")
            .map_err(|e| StoreError::Db(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(rows.collect::<Result<Vec<_>, _>>().map_err(|e| StoreError::Db(e.to_string()))?)
    }

    async fn set_hidden(&self, addr: &str, hidden: bool) -> Result<(), StoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StoreError::Db(e.to_string()))?;
        if hidden {
            conn.execute(
                "INSERT OR IGNORE INTO hidden_addresses (address) VALUES (?1)",
                rusqlite::params![addr],
            )
            .map_err(|e| StoreError::Db(e.to_string()))?;
        } else {
            conn.execute(
                "DELETE FROM hidden_addresses WHERE address = ?1",
                rusqlite::params![addr],
            )
            .map_err(|e| StoreError::Db(e.to_string()))?;
        }
        Ok(())
    }

    async fn get_hidden_addresses(&self) -> Result<Vec<String>, StoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StoreError::Db(e.to_string()))?;
        let mut stmt = conn
            .prepare("SELECT address FROM hidden_addresses")
            .map_err(|e| StoreError::Db(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(rows.collect::<Result<Vec<_>, _>>().map_err(|e| StoreError::Db(e.to_string()))?)
    }

    async fn delete_messages_for_address(&self, addr: &str) -> Result<(), StoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StoreError::Db(e.to_string()))?;
        conn.execute(
            "DELETE FROM messages WHERE from_addr = ?1 OR to_addr = ?1",
            rusqlite::params![addr],
        )
        .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(())
    }

    async fn get_unique_addresses(&self) -> Result<Vec<String>, StoreError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| StoreError::Db(e.to_string()))?;
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT addr FROM (
                    SELECT from_addr AS addr FROM messages
                    UNION
                    SELECT to_addr AS addr FROM messages
                )",
            )
            .map_err(|e| StoreError::Db(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(rows.collect::<Result<Vec<_>, _>>().map_err(|e| StoreError::Db(e.to_string()))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sqlite_crud() {
        let db = SqliteStorage::open(":memory:").unwrap();

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let msg = Message::new(
                "alice@yse.org".into(),
                "bob@yse.org".into(),
                Some("hi".into()),
            );
            db.save_message(&msg).await.unwrap();

            assert!(!db.is_processed(&msg.id).await.unwrap());
            db.mark_processed(&msg.id).await.unwrap();
            assert!(db.is_processed(&msg.id).await.unwrap());

            let msgs = db.list_messages(10, 0).await.unwrap();
            assert_eq!(msgs.len(), 1);
            assert_eq!(msgs[0].id, msg.id);

            // plugin
            let plugin = PluginConfig {
                id: "test".into(),
                name: "Test Bot".into(),
                exec_path: "/usr/bin/echo".into(),
                args: vec!["hello".into()],
                enabled: true,
            };
            db.save_plugin(&plugin).await.unwrap();
            let plugins = db.list_plugins().await.unwrap();
            assert_eq!(plugins.len(), 1);

            // config
            db.set_config_value("key1", "val1").await.unwrap();
            assert_eq!(
                db.get_config_value("key1").await.unwrap(),
                Some("val1".into())
            );
        });
    }

    #[test]
    fn test_processed_survives_deletion() {
        let msg = crate::message::Message::new(
            "alice#abc@host".into(),
            "bob#def@host".into(),
            Some("test".into()),
        );
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let db = SqliteStorage::open(":memory:").unwrap();
            db.save_message(&msg).await.unwrap();
            db.mark_processed(&msg.id).await.unwrap();
            assert!(db.is_processed(&msg.id).await.unwrap());

            db.delete_messages_for_address("alice#abc@host").await.unwrap();

            assert!(
                db.is_processed(&msg.id).await.unwrap(),
                "processed mark must survive message deletion"
            );
        });
    }
}
