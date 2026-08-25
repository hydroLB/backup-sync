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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyWarning {
    pub ts: i64,
    pub message: String,
    #[serde(default)]
    pub watched_path: Option<String>,
    #[serde(default, alias = "pinned_version_id")]
    pub kept_version_id: Option<String>,
}

impl Default for ActivityItem {
    /// Ensure new entries have a consistent timestamp baseline.
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
    #[serde(default)]
    pub cycles_since_full_scan: u64,
    pub start_ts: Option<i64>,
    pub version: Option<String>,
    pub last_verify_ts: Option<i64>,
    pub last_verify_status: Option<String>,
    pub last_verify_issues: Option<usize>,
    #[serde(default)]
    pub last_scrub_full_ts: Option<i64>,
    pub recent_activity: Vec<ActivityItem>,
    pub safe_mode: bool,
    #[serde(default)]
    pub destination_paused: bool,
    #[serde(default)]
    pub destination_pause_reason: Option<String>,
    #[serde(default)]
    pub destination_unavailable_ids: Vec<String>,
    #[serde(default)]
    pub destination_last_unavailable_ts: Option<i64>,
    #[serde(default)]
    pub destination_last_recovered_ts: Option<i64>,
    #[serde(default)]
    pub replication_last_run_ts: Option<i64>,
    #[serde(default)]
    pub replication_last_status: Option<String>,
    #[serde(default)]
    pub replication_last_error: Option<String>,
    #[serde(default)]
    pub replication_last_bytes_copied: u64,
    #[serde(default)]
    pub replication_last_blobs_copied: usize,
    #[serde(default)]
    pub replication_last_manifests_copied: usize,
    #[serde(default)]
    pub replication_last_manifests_deleted: usize,
    #[serde(default)]
    pub replication_last_pairs_ok: usize,
    #[serde(default)]
    pub replication_last_pairs_failed: usize,
    #[serde(default)]
    pub replication_last_targets_failed: Vec<String>,
    #[serde(default)]
    pub last_safety_warning: Option<SafetyWarning>,
}
