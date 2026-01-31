use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileState {
    pub len: u64,
    pub mtime: i64,
    pub last_hash: Option<String>,
    pub backups: Vec<PathBuf>,
    pub stable_cycles: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityItem {
    pub path: String,
    pub bytes: u64,
    pub ts: i64,
}

impl Default for ActivityItem {
    /// Purpose: Builds a default activity item with a current timestamp.
    ///
    /// Inputs: none.
    /// Outputs: a default `ActivityItem`.
    /// Ties to: state initialization and UI activity feeds.
    /// Side effects: None.
    /// Why: ensure new entries have a consistent timestamp baseline.
    fn default() -> Self {
        Self {
            path: String::new(),
            bytes: 0,
            ts: Utc::now().timestamp(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoredState {
    pub files: HashMap<String, FileState>,
    pub last_run_ts: Option<i64>,
    pub last_error: Option<String>,
    pub last_files_backed_up: usize,
    pub prune_counter: u64,
    pub last_dirty_count: usize,
    pub start_ts: Option<i64>,
    pub version: Option<String>,
    pub last_verify_ts: Option<i64>,
    pub last_verify_status: Option<String>,
    pub last_verify_issues: Option<usize>,
    pub recent_activity: Vec<ActivityItem>,
    pub safe_mode: bool,
}
