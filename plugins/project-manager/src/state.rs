use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub text: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PState {
    Init,
    Proposing,
    Proposed { proposal: Proposal },
    Approved { proposal: Proposal, since: u64 },
    Completed { last_proposal: String },
    Sleeping {
        proposal: Proposal,
        sleep_start: u64,
        hours_slept: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub proposal: String,
    pub result: String,
    pub completed: bool,
    pub feedback: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectState {
    pub project_dir: Option<String>,
    pub state: PState,
    pub history: Vec<HistoryEntry>,
}

impl Default for ProjectState {
    fn default() -> Self {
        Self {
            project_dir: None,
            state: PState::Init,
            history: Vec::new(),
        }
    }
}

impl ProjectState {
    pub async fn load(state_dir: &Path) -> Self {
        let path = state_dir.join("state.json");
        match fs::read_to_string(&path).await {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub async fn save(&self, state_dir: &Path) {
        let path = state_dir.join("state.json");
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = fs::write(&path, &content).await;
        }
    }
}
