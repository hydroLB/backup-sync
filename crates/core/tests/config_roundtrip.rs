use backup_core::{
    config::model::{Config, ExecutionTuning, HashingTuning, PlanningTuning},
    load_from_path,
};
use std::fs;
use tempfile::tempdir;

#[test]
/// Purpose: Ensures config serialization and loading round-trip correctly.
///
/// Inputs: a serialized config written to disk.
/// Outputs: a loaded config matching core fields.
/// Ties to: TOML config persistence.
/// Side effects: None.
/// Why: verify config load paths preserve critical settings.
fn config_roundtrip() {
    let dir = tempdir().expect("config_roundtrip::config_roundtrip failed to create temp dir");
    let path = dir.path().join("cfg.toml");
    let cfg = Config {
        backup_root: dir.path().join("b"),
        interval_seconds: 60,
        max_backups_per_file: 3,
        skip_hidden: true,
        ignore_patterns: vec![],
        max_parallel_copies: 1,
        max_bytes_per_second: None,
        min_free_space_bytes: None,
        hashing: HashingTuning::default(),
        execution: ExecutionTuning::default(),
        planning: PlanningTuning::default(),
        runtime: backup_core::config::model::RuntimeTuning::default(),
        safe_mode: false,
        watched: vec![],
        destinations: vec![backup_core::config::model::Destination {
            id: "default".into(),
            path: dir.path().join("b"),
            label: None,
            max_backups_per_file: None,
        }],
    };
    let raw =
        toml::to_string(&cfg).expect("config_roundtrip::config_roundtrip failed to serialize");
    fs::write(&path, raw).expect("config_roundtrip::config_roundtrip failed to write config file");
    let loaded =
        load_from_path(&path).expect("config_roundtrip::config_roundtrip failed to load config");
    assert_eq!(loaded.backup_root, cfg.backup_root);
    assert_eq!(loaded.interval_seconds, 60);
}
