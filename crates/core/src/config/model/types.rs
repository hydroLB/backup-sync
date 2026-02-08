use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::tuning::{ExecutionTuning, HashingTuning, PlanningTuning, RuntimeTuning};
use super::{CompressionConfig, EncryptionConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WatchedKind {
    File,
    Directory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Destination {
    pub id: String,
    pub path: PathBuf,
    pub label: Option<String>,
    #[serde(default)]
    pub max_backups_per_file: Option<usize>,
    #[serde(default)]
    pub replicate_to: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchedPath {
    pub path: PathBuf,
    pub kind: WatchedKind,
    pub enabled: bool,
    #[serde(default = "crate::config::model::defaults::default_destination_id")]
    pub destination_id: String,
    #[serde(default)]
    pub max_backups_per_file: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub backup_root: PathBuf,
    pub interval_seconds: u64,
    pub max_backups_per_file: usize,
    #[serde(default = "crate::config::model::defaults::default_skip_hidden")]
    pub skip_hidden: bool,
    #[serde(default = "crate::config::model::defaults::default_ignore_patterns")]
    pub ignore_patterns: Vec<String>,
    #[serde(default = "crate::config::model::defaults::default_max_parallel_copies")]
    pub max_parallel_copies: usize,
    #[serde(default = "crate::config::model::defaults::default_max_bytes_per_second")]
    pub max_bytes_per_second: Option<u64>,
    #[serde(default = "crate::config::model::defaults::default_min_free_space_bytes")]
    pub min_free_space_bytes: Option<u64>,
    #[serde(default)]
    pub hashing: HashingTuning,
    #[serde(default)]
    pub execution: ExecutionTuning,
    #[serde(default)]
    pub planning: PlanningTuning,
    #[serde(default)]
    pub runtime: RuntimeTuning,
    #[serde(default)]
    pub safe_mode: bool,
    #[serde(default)]
    pub encryption: EncryptionConfig,
    #[serde(default)]
    pub compression: CompressionConfig,
    pub watched: Vec<WatchedPath>,
    #[serde(default)]
    pub destinations: Vec<Destination>,
}
