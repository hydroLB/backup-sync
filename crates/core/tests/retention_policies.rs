use backup_core::{
    backup::{execution::BackupExecutor, planning, retention::policy::enforce},
    config::model::{
        Config, ExecutionTuning, HashingTuning, PlanningTuning, WatchedKind, WatchedPath,
    },
    fs::scanning::collect_targets,
    state::store::StateStore,
};
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
/// Avoid duplicating backups when content is stable.
fn unchanged_files_not_backed_up_twice() {
    let dir = tempdir().expect(
        "retention_policies::unchanged_files_not_backed_up_twice failed to create temp dir",
    );
    let file = dir.path().join("b.txt");
    fs::write(&file, "hello")
        .expect("retention_policies::unchanged_files_not_backed_up_twice failed to write file");
    let cfg = Config {
        backup_root: dir.path().join("backups"),
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
        encryption: backup_core::config::model::EncryptionConfig::default(),
        compression: backup_core::config::model::CompressionConfig::default(),
        safe_mode: false,
        watched: vec![WatchedPath {
            path: file.clone(),
            kind: WatchedKind::File,
            enabled: true,
            destination_id: "default".into(),
            max_backups_per_file: None,
        }],
        destinations: vec![backup_core::config::model::Destination {
            id: "default".into(),
            path: dir.path().join("backups"),
            label: None,
            max_backups_per_file: None,
            replicate_to: vec![],
        }],
    };
    let (mut state, _) = StateStore::load_or_default(dir.path().join("state.json"))
        .expect("retention_policies::unchanged_files_not_backed_up_twice failed to load state");

    let targets = collect_targets(&cfg).expect(
        "retention_policies::unchanged_files_not_backed_up_twice failed to collect targets",
    );
    let plan = planning::plan(targets, &mut state, &cfg.planning, &cfg.hashing)
        .expect("retention_policies::unchanged_files_not_backed_up_twice failed to build plan");
    let exec = BackupExecutor::from_config(&cfg);
    exec.execute(&plan, &mut state)
        .expect("retention_policies::unchanged_files_not_backed_up_twice failed to execute plan");
    let initial = state
        .files
        .values()
        .next()
        .expect("retention_policies::unchanged_files_not_backed_up_twice missing state entry")
        .backups
        .len();

    let targets2 = collect_targets(&cfg).expect(
        "retention_policies::unchanged_files_not_backed_up_twice failed to collect targets on second run",
    );
    let plan2 = planning::plan(targets2, &mut state, &cfg.planning, &cfg.hashing).expect(
        "retention_policies::unchanged_files_not_backed_up_twice failed to build plan on second run",
    );
    let exec = BackupExecutor::from_config(&cfg);
    exec.execute(&plan2, &mut state).expect(
        "retention_policies::unchanged_files_not_backed_up_twice failed to execute plan on second run",
    );
    let after = state
        .files
        .values()
        .next()
        .expect("retention_policies::unchanged_files_not_backed_up_twice missing state entry on second run")
        .backups
        .len();
    assert_eq!(initial, after);
}

#[test]
/// Preserve expected newest backup when enforcing retention.
fn retention_sorts_by_timestamp_prefix() {
    let files = vec![
        PathBuf::from("20240102-010000__a.txt"),
        PathBuf::from("20230101-010000__a.txt"),
        PathBuf::from("20240101-010000__a.txt"),
    ];
    let kept = enforce(1, files)
        .expect("retention_policies::retention_sorts_by_timestamp_prefix failed to enforce");
    assert_eq!(kept.len(), 1);
    assert!(kept[0].to_string_lossy().contains("20240102-010000"));
}

#[test]
/// Keep at least one copy per file.
fn retention_never_deletes_last_when_max_one() {
    let files = vec![PathBuf::from("20240101-010000__a.txt")];
    let kept = enforce(1, files.clone())
        .expect("retention_policies::retention_never_deletes_last_when_max_one failed to enforce");
    assert_eq!(kept.len(), 1);
}
