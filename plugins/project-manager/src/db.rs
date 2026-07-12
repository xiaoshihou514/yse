use rusqlite::{params, Connection, Result as SqlResult};
use std::path::Path;

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(db_path: &Path) -> SqlResult<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS project_state (
                project_dir TEXT PRIMARY KEY,
                state_json  TEXT NOT NULL,
                updated_at  INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS conversation (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                project_dir TEXT NOT NULL,
                role        TEXT NOT NULL,
                content     TEXT NOT NULL,
                turn        INTEGER NOT NULL,
                created_at  INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_conv_proj ON conversation(project_dir);

            CREATE TABLE IF NOT EXISTS repo_cache (
                project_dir     TEXT PRIMARY KEY,
                commit_hash     TEXT NOT NULL,
                structure_blob  TEXT NOT NULL,
                created_at      INTEGER NOT NULL
            );
            ",
        )?;
        Ok(Self { conn })
    }

    pub fn save_project_state(&self, project_dir: &str, state_json: &str) -> SqlResult<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.conn.execute(
            "INSERT INTO project_state (project_dir, state_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(project_dir) DO UPDATE SET
               state_json = excluded.state_json,
               updated_at = excluded.updated_at",
            params![project_dir, state_json, now],
        )?;
        Ok(())
    }

    pub fn load_project_state(&self, project_dir: &str) -> SqlResult<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT state_json FROM project_state WHERE project_dir = ?1")?;
        let mut rows = stmt.query(params![project_dir])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    pub fn add_conversation_turn(
        &self,
        project_dir: &str,
        role: &str,
        content: &str,
        turn: u64,
    ) -> SqlResult<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.conn.execute(
            "INSERT INTO conversation (project_dir, role, content, turn, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![project_dir, role, content, turn, now],
        )?;
        Ok(())
    }

    pub fn load_history(
        &self,
        project_dir: &str,
        max_turns: u64,
    ) -> SqlResult<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT role, content FROM conversation
             WHERE project_dir = ?1 AND turn >= (
                 SELECT COALESCE(MAX(turn) - ?2, 0) FROM conversation WHERE project_dir = ?1
             )
             ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![project_dir, max_turns], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    pub fn load_cached_structure(&self, project_dir: &str) -> SqlResult<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT structure_blob FROM repo_cache WHERE project_dir = ?1")?;
        let mut rows = stmt.query(params![project_dir])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    pub fn load_cached_commit(&self, project_dir: &str) -> SqlResult<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT commit_hash FROM repo_cache WHERE project_dir = ?1")?;
        let mut rows = stmt.query(params![project_dir])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    pub fn save_structure_cache(
        &self,
        project_dir: &str,
        commit_hash: &str,
        structure_blob: &str,
    ) -> SqlResult<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.conn.execute(
            "INSERT INTO repo_cache (project_dir, commit_hash, structure_blob, created_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(project_dir) DO UPDATE SET
               commit_hash = excluded.commit_hash,
               structure_blob = excluded.structure_blob,
               created_at = excluded.created_at",
            params![project_dir, commit_hash, structure_blob, now],
        )?;
        Ok(())
    }

    pub fn load_first_project_dir(&self) -> SqlResult<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT project_dir FROM project_state LIMIT 1")?;
        let mut rows = stmt.query([])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    pub fn get_max_turn(&self, project_dir: &str) -> SqlResult<u64> {
        let mut stmt = self
            .conn
            .prepare("SELECT COALESCE(MAX(turn), 0) FROM conversation WHERE project_dir = ?1")?;
        let mut rows = stmt.query(params![project_dir])?;
        match rows.next()? {
            Some(row) => Ok(row.get(0)?),
            None => Ok(0),
        }
    }
}
