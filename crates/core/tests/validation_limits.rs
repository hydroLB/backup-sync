use backup_core::{
    backup::{
        execution::BackupExecutor,
        planning::{enforce_plan_limits, PlannedItem},
    },
    config::{
        model::{Config, ExecutionTuning, HashingTuning, PlanningTuning, WatchedKind, WatchedPath},
        validate,
    },
    state::models::StoredState,
};
use std::{fs, time::SystemTime};
use tempfile::tempdir;

/// Purpose: Builds a baseline config for validation tests with a single destination.
///
/// Inputs: the backup root and watched path list.
/// Outputs: a fully populated `Config` with safe defaults.
/// Ties to: config validation and plan limit tests.
/// Side effects: None.
/// Why: keep test setup consistent across validation scenarios.
fn base_config(backup_root: std::path::PathBuf, watched: Vec<WatchedPath>) -> Config {
    let watched = watched
        .into_iter()
        .map(|mut w| {
            if w.destination_id.is_empty() {
                w.destination_id = "default".into();
            }
            w
        })
        .collect();
    Config {
        backup_root: backup_root.clone(),
        interval_seconds: 10,
        max_backups_per_file: 5,
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
        watched,
        destinations: vec![backup_core::config::model::Destination {
            id: "default".into(),
            path: backup_root,
            label: None,
            max_backups_per_file: None,
        }],
    }
}

#[test]
/// Purpose: Ensures validation allows configs without watched paths.
///
/// Inputs: a temp directory and an empty watched list.
/// Outputs: a successful validation result.
/// Ties to: config validation error handling.
/// Side effects: None.
/// Why: allow users to set destinations and schedules before selecting folders.
fn validate_allows_empty_watched_list() {
    let dir = tempdir()
        .expect("validation_limits::validate_allows_empty_watched_list failed to create temp dir");
    let cfg = base_config(dir.path().join("backups"), vec![]);
    assert!(
        validate::validate(&cfg).is_ok(),
        "expected validation to succeed when watched list is empty"
    );
}

#[test]
/// Purpose: Ensures validation rejects backup_root when it points to a file.
///
/// Inputs: a temp directory and a file path as backup_root.
/// Outputs: a failed validation result.
/// Ties to: filesystem sanity checks during validation.
/// Side effects: None.
/// Why: avoid writing backups into a regular file.
fn validate_rejects_backup_root_that_is_file() {
    let dir = tempdir().expect(
        "validation_limits::validate_rejects_backup_root_that_is_file failed to create temp dir",
    );
    let backup_root = dir.path().join("dest.txt");
    fs::write(&backup_root, "not a dir").expect(
        "validation_limits::validate_rejects_backup_root_that_is_file failed to write backup_root",
    );
    let watched_path = dir.path().join("watched.txt");
    fs::write(&watched_path, "data").expect(
        "validation_limits::validate_rejects_backup_root_that_is_file failed to write watched file",
    );
    let cfg = base_config(
        backup_root,
        vec![WatchedPath {
            path: watched_path,
            kind: WatchedKind::File,
            enabled: true,
            destination_id: "default".into(),
            max_backups_per_file: None,
        }],
    );
    assert!(
        validate::validate(&cfg).is_err(),
        "should reject backup_root that points to a file"
    );
}

#[test]
/// Purpose: Ensures oversized plans are rejected by planning limits.
///
/// Inputs: a plan with more items than the configured maximum.
/// Outputs: a failed plan limit check.
/// Ties to: `enforce_plan_limits` guardrails.
/// Side effects: None.
/// Why: keep cycles bounded to avoid runaway IO.
fn plan_limit_is_enforced() {
    let item = PlannedItem {
        src: std::path::PathBuf::from("/tmp/file"),
        len: 1,
        mtime: 0,
        reason: "test".into(),
        precomputed_hash: None,
        key: "k".into(),
        destination_root: std::path::PathBuf::from("/tmp/dest"),
        max_copies: 1,
    };
    let mut plan = Vec::new();
    let tuning = PlanningTuning::default();
    plan.resize(tuning.max_plan_items + 1, item);
    assert!(
        enforce_plan_limits(&plan, &tuning).is_err(),
        "plan should fail when exceeding MAX_PLAN_ITEMS"
    );
}

#[test]
/// Purpose: Ensures validation rejects unwritable destination targets.
///
/// Inputs: a destination path that is a file and a valid watched path.
/// Outputs: a failed validation result.
/// Ties to: destination checks for writability.
/// Side effects: None.
/// Why: prevent writing backups to non-directory paths.
fn unwritable_destination_is_rejected() {
    let dir = tempdir()
        .expect("validation_limits::unwritable_destination_is_rejected failed to create temp dir");
    let backup_root = dir.path().join("backups");
    // Create parent but make backup dir a file to simulate unwritable target
    std::fs::write(&backup_root, "not a directory").expect(
        "validation_limits::unwritable_destination_is_rejected failed to write backup_root",
    );
    let watched_path = dir.path().join("watched.txt");
    std::fs::write(&watched_path, "data").expect(
        "validation_limits::unwritable_destination_is_rejected failed to write watched file",
    );
    let cfg = base_config(
        backup_root,
        vec![WatchedPath {
            path: watched_path,
            kind: WatchedKind::File,
            enabled: true,
            destination_id: "default".into(),
            max_backups_per_file: None,
        }],
    );
    assert!(
        validate::validate(&cfg).is_err(),
        "should reject backup_root that is not a writable directory"
    );
}

#[test]
/// Purpose: Ensures the free space guard blocks execution when below the configured minimum.
///
/// Inputs: a temp backup plan and an extreme min_free_space_bytes.
/// Outputs: a result with a recorded error in state.
/// Ties to: `BackupExecutor::ensure_free_space` guard logic.
/// Side effects: None.
/// Why: prevent starting copies when the target has insufficient space.
fn min_free_space_blocks_execution() {
    let backups = tempdir()
        .expect("validation_limits::min_free_space_blocks_execution failed to create backups dir");
    let src_dir = tempdir()
        .expect("validation_limits::min_free_space_blocks_execution failed to create src dir");
    let src = src_dir.path().join("file.txt");
    fs::write(&src, "hello")
        .expect("validation_limits::min_free_space_blocks_execution failed to write source file");
    let meta = fs::metadata(&src).expect(
        "validation_limits::min_free_space_blocks_execution failed to read source metadata",
    );
    let mtime = meta
        .modified()
        .ok()
        .and_then(|m| m.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let plan = vec![PlannedItem {
        src: src.clone(),
        len: meta.len(),
        mtime,
        reason: "test".into(),
        precomputed_hash: None,
        key: src.to_string_lossy().into_owned(),
        destination_root: backups.path().to_path_buf(),
        max_copies: 1,
    }];
    let mut state = StoredState::default();
    let exec = BackupExecutor {
        max_parallel_copies: 1,
        max_bytes_per_second: None,
        min_free_space_bytes: Some(u64::MAX / 2),
        tuning: ExecutionTuning::default(),
        hashing: HashingTuning::default(),
    };
    let result = exec
        .execute(&plan, &mut state)
        .expect("validation_limits::min_free_space_blocks_execution failed to execute plan");
    assert_eq!(result.backed_up, 0);
    assert_eq!(result.errors, 1);
    let err = state.last_error.as_deref().unwrap_or_default();
    assert!(
        err.contains("below configured minimum"),
        "unexpected error: {}",
        err
    );
}

#[test]
/// Purpose: Ensures invalid ignore patterns are rejected by validation.
///
/// Inputs: a config with a malformed ignore pattern.
/// Outputs: a failed validation result.
/// Ties to: glob parsing checks.
/// Side effects: None.
/// Why: avoid silently ignoring invalid patterns.
fn invalid_ignore_pattern_rejected() {
    let dir = tempdir()
        .expect("validation_limits::invalid_ignore_pattern_rejected failed to create temp dir");
    let watched = dir.path().join("watched.txt");
    fs::write(&watched, "data")
        .expect("validation_limits::invalid_ignore_pattern_rejected failed to write watched file");
    let mut cfg = base_config(
        dir.path().join("backups"),
        vec![WatchedPath {
            path: watched,
            kind: WatchedKind::File,
            enabled: true,
            destination_id: "default".into(),
            max_backups_per_file: None,
        }],
    );
    cfg.ignore_patterns = vec!["[unclosed".into()];
    assert!(
        validate::validate(&cfg).is_err(),
        "invalid glob should be rejected"
    );
}
