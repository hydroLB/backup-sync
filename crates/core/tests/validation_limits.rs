use backup_core::config::{
    model::{Config, ExecutionTuning, HashingTuning, PlanningTuning, WatchedKind, WatchedPath},
    validate,
};
use std::fs;
use tempfile::tempdir;

/// Keep test setup consistent across validation scenarios.
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
        encryption: backup_core::config::model::EncryptionConfig::default(),
        compression: backup_core::config::model::CompressionConfig::default(),
        safe_mode: false,
        watched,
        destinations: vec![backup_core::config::model::Destination {
            id: "default".into(),
            path: backup_root,
            label: None,
            max_backups_per_file: None,
            replicate_to: vec![],
        }],
    }
}

#[test]
/// Allow users to set destinations and schedules before selecting folders.
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
/// Avoid writing backups into a regular file.
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
/// Keep cycles bounded to avoid runaway IO.
fn plan_limit_is_enforced() {
    execution_planning_cases::plan_limit_is_enforced_impl();
}

#[test]
/// Prevent writing backups to non-directory paths.
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
/// Prevent starting copies when the target has insufficient space.
fn min_free_space_blocks_execution() {
    execution_planning_cases::min_free_space_blocks_execution_impl();
}

mod execution_planning_cases {
    use super::*;
    use backup_core::backup::{
        execution::BackupExecutor,
        planning::{enforce_plan_limits, PlannedItem},
    };
    use backup_core::state::models::StoredState;
    use std::time::SystemTime;

    pub fn plan_limit_is_enforced_impl() {
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

    pub fn min_free_space_blocks_execution_impl() {
        let backups = tempdir().expect(
            "validation_limits::min_free_space_blocks_execution failed to create backups dir",
        );
        let src_dir = tempdir()
            .expect("validation_limits::min_free_space_blocks_execution failed to create src dir");
        let src = src_dir.path().join("file.txt");
        fs::write(&src, "hello").expect(
            "validation_limits::min_free_space_blocks_execution failed to write source file",
        );
        let meta = fs::metadata(&src).expect(
            "validation_limits::min_free_space_blocks_execution failed to read source metadata",
        );
        let mtime = match meta.modified() {
            Ok(modified) => match modified.duration_since(SystemTime::UNIX_EPOCH) {
                Ok(duration) => duration.as_secs() as i64,
                Err(_) => 0,
            },
            Err(_) => 0,
        };

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
}

#[test]
/// Avoid silently ignoring invalid patterns.
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
