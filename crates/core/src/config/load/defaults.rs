use crate::config::model::{self, Config, ExecutionTuning, PlanningTuning, RuntimeTuning};
use anyhow::{Context, Result};

/// Keep startup predictable with safe baseline settings.
pub fn default_config() -> Result<Config> {
    let backup_root = crate::platform::paths::default_backup_root()
        .context("config::default_config failed to resolve default backup root")?;
    Ok(Config {
        backup_root: backup_root.clone(),
        interval_seconds: model::default_interval_seconds(),
        max_backups_per_file: model::default_max_backups_per_file(),
        skip_hidden: model::default_skip_hidden(),
        ignore_patterns: model::default_ignore_patterns(),
        max_parallel_copies: model::default_max_parallel_copies(),
        max_bytes_per_second: model::default_max_bytes_per_second(),
        min_free_space_bytes: model::default_min_free_space_bytes(),
        hashing: model::HashingTuning::default(),
        execution: ExecutionTuning::default(),
        planning: PlanningTuning::default(),
        runtime: RuntimeTuning::default(),
        safe_mode: false,
        encryption: model::EncryptionConfig::default(),
        compression: model::CompressionConfig::default(),
        watched: vec![],
        destinations: vec![crate::config::model::Destination {
            id: model::default_destination_id(),
            path: backup_root,
            label: Some("Primary".into()),
            max_backups_per_file: None,
            replicate_to: vec![],
        }],
    })
}
