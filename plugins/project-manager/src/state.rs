use serde::{Deserialize, Serialize};

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
    pub turn: u64,
}

impl Default for ProjectState {
    fn default() -> Self {
        Self {
            project_dir: None,
            state: PState::Init,
            history: Vec::new(),
            turn: 0,
        }
    }
}
