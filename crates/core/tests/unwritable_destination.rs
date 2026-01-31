use backup_core::{
    backup::{execution::BackupExecutor, planning},
    config::model::{
        Config, ExecutionTuning, HashingTuning, PlanningTuning, WatchedKind, WatchedPath,
    },
    fs::scanning::collect_targets,
    state::store::StateStore,
    validate,
};
use std::{fs, path::PathBuf};
use tempfile::tempdir;

/// Purpose: Builds a config with a single destination and watched path.
///
/// Inputs: the destination path and watched file path.
/// Outputs: a fully populated config with defaults.
/// Ties to: validation and execution tests.
/// Side effects: None.
/// Why: centralize test config creation for reuse.
fn config_with_dest(dest: PathBuf, watched: PathBuf) -> Config {
    Config {
        backup_root: dest.clone(),
        interval_seconds: 10,
        max_backups_per_file: 1,
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
        watched: vec![WatchedPath {
            path: watched,
            kind: WatchedKind::File,
            enabled: true,
            destination_id: "default".into(),
            max_backups_per_file: None,
        }],
        destinations: vec![backup_core::config::model::Destination {
            id: "default".into(),
            path: dest,
            label: None,
            max_backups_per_file: None,
        }],
    }
}

#[test]
/// Purpose: Ensures validation fails when destination is not a directory.
///
/// Inputs: a file path used as destination.
/// Outputs: a validation error.
/// Ties to: destination path validation.
/// Side effects: None.
/// Why: prevent backups from targeting files instead of directories.
fn fails_when_destination_unwritable() {
    let dir = tempdir().expect(
        "unwritable_destination::fails_when_destination_unwritable failed to create temp dir",
    );
    let watched = dir.path().join("file.txt");
    fs::write(&watched, "data").expect(
        "unwritable_destination::fails_when_destination_unwritable failed to write watched file",
    );

    // Point destination to a file to trigger unwritable validation.
    let dest = dir.path().join("dest-as-file.txt");
    fs::write(&dest, "not a dir").expect(
        "unwritable_destination::fails_when_destination_unwritable failed to write dest file",
    );
    let cfg = config_with_dest(dest.clone(), watched);
    assert!(
        validate(&cfg).is_err(),
        "expected validation to fail when dest is a file"
    );
}

#[test]
/// Purpose: Ensures execution reports an error when free space is below the minimum.
///
/// Inputs: a plan and a min_free_space_bytes larger than available.
/// Outputs: an execution result with errors recorded.
/// Ties to: executor free space guard logic.
/// Side effects: None.
/// Why: avoid starting backups that cannot complete.
fn min_free_space_stops_run() {
    let dir = tempdir()
        .expect("unwritable_destination::min_free_space_stops_run failed to create temp dir");
    let watched = dir.path().join("file.txt");
    fs::write(&watched, "hello")
        .expect("unwritable_destination::min_free_space_stops_run failed to write watched file");
    let dest = dir.path().join("backups");
    let cfg = Config {
        backup_root: dest.clone(),
        interval_seconds: 10,
        max_backups_per_file: 1,
        skip_hidden: true,
        ignore_patterns: vec![],
        max_parallel_copies: 1,
        max_bytes_per_second: None,
        min_free_space_bytes: Some(u64::MAX / 2),
        hashing: HashingTuning::default(),
        execution: ExecutionTuning::default(),
        planning: PlanningTuning::default(),
        runtime: backup_core::config::model::RuntimeTuning::default(),
        safe_mode: false,
        watched: vec![WatchedPath {
            path: watched.clone(),
            kind: WatchedKind::File,
            enabled: true,
            destination_id: "default".into(),
            max_backups_per_file: None,
        }],
        destinations: vec![backup_core::config::model::Destination {
            id: "default".into(),
            path: dest.clone(),
            label: None,
            max_backups_per_file: None,
        }],
    };
    let (mut state, _) = StateStore::load_or_default(dir.path().join("state.json"))
        .expect("unwritable_destination::min_free_space_stops_run failed to load state store");
    let targets = collect_targets(&cfg)
        .expect("unwritable_destination::min_free_space_stops_run failed to collect targets");
    let plan = planning::plan(targets, &mut state, &cfg.planning, &cfg.hashing)
        .expect("unwritable_destination::min_free_space_stops_run failed to build plan");
    let exec = BackupExecutor {
        max_parallel_copies: cfg.max_parallel_copies,
        max_bytes_per_second: cfg.max_bytes_per_second,
        min_free_space_bytes: cfg.min_free_space_bytes,
        tuning: ExecutionTuning::default(),
        hashing: HashingTuning::default(),
    };
    let res = exec
        .execute(&plan, &mut state)
        .expect("unwritable_destination::min_free_space_stops_run failed to execute plan");
    assert_eq!(res.backed_up, 0);
    assert_eq!(res.errors, 1);
}
