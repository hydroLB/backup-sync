use backup_core::backup::versioned::{run_backup_cycle, simulate_backup_cycle_with_sample_limit};
use backup_core::config::model::{
    Config, Destination, ExecutionTuning, HashingTuning, PlanningTuning, RuntimeTuning,
    WatchedKind, WatchedPath,
};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

/// Summary: build_cfg orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
fn build_cfg(source: &Path, dest: &Path) -> Config {
    Config {
        backup_root: dest.to_path_buf(),
        interval_seconds: 1800,
        max_backups_per_file: 5,
        skip_hidden: true,
        ignore_patterns: vec![],
        max_parallel_copies: 1,
        max_bytes_per_second: None,
        min_free_space_bytes: None,
        hashing: HashingTuning::default(),
        execution: ExecutionTuning::default(),
        planning: PlanningTuning::default(),
        runtime: RuntimeTuning::default(),
        encryption: backup_core::config::model::EncryptionConfig::default(),
        compression: backup_core::config::model::CompressionConfig::default(),
        safe_mode: false,
        watched: vec![WatchedPath {
            path: source.to_path_buf(),
            kind: WatchedKind::Directory,
            enabled: true,
            destination_id: "default".into(),
            max_backups_per_file: Some(5),
        }],
        destinations: vec![Destination {
            id: "default".into(),
            path: dest.to_path_buf(),
            label: None,
            max_backups_per_file: None,
            replicate_to: vec![],
        }],
    }
}

/// Summary: simulate_reports_changes_and_dedupe_savings orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
#[test]
fn simulate_reports_changes_and_dedupe_savings() {
    let tmp = tempdir().expect("versioned_simulate::simulate_reports_changes tempdir");
    let source = tmp.path().join("source");
    let dest = tmp.path().join("dest");
    fs::create_dir_all(&source).expect("versioned_simulate create source");
    fs::create_dir_all(&dest).expect("versioned_simulate create dest");

    let a = source.join("a.txt");
    fs::write(&a, "one").expect("versioned_simulate write a v1");
    let cfg = build_cfg(&source, &dest);

    run_backup_cycle(&cfg).expect("versioned_simulate initial backup");

    let sim0 = simulate_backup_cycle_with_sample_limit(&cfg, 5)
        .expect("versioned_simulate simulate no changes");
    assert_eq!(sim0.items, 0);
    assert_eq!(sim0.bytes_to_write, 0);
    assert_eq!(sim0.versions_would_create, 0);

    fs::write(&a, "two").expect("versioned_simulate write a v2");
    let sim1 = simulate_backup_cycle_with_sample_limit(&cfg, 5)
        .expect("versioned_simulate simulate modify");
    assert_eq!(sim1.adds, 0);
    assert_eq!(sim1.modifies, 1);
    assert_eq!(sim1.deletes, 0);
    assert_eq!(sim1.items, 1);
    assert_eq!(sim1.blobs_to_write, 1);
    assert_eq!(sim1.bytes_to_write, 3);
    assert!(
        sim1.sample.iter().any(|s| s.starts_with("~ ")),
        "expected a modify sample entry"
    );

    run_backup_cycle(&cfg).expect("versioned_simulate commit v2");

    // Reverting content should still be a logical change, but should not require writing a blob if
    // the earlier content hash already exists in the store.
    fs::write(&a, "one").expect("versioned_simulate revert a v1");
    let sim2 = simulate_backup_cycle_with_sample_limit(&cfg, 5)
        .expect("versioned_simulate simulate revert");
    assert_eq!(sim2.items, 1);
    assert_eq!(sim2.modifies, 1);
    assert_eq!(sim2.blobs_to_write, 0);
    assert_eq!(sim2.bytes_to_write, 0);
}

/// Summary: simulate_counts_adds_and_deletes orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
#[test]
fn simulate_counts_adds_and_deletes() {
    let tmp = tempdir().expect("versioned_simulate::simulate_counts_adds tempdir");
    let source = tmp.path().join("source");
    let dest = tmp.path().join("dest");
    fs::create_dir_all(&source).expect("versioned_simulate add/delete create source");
    fs::create_dir_all(&dest).expect("versioned_simulate add/delete create dest");

    fs::write(source.join("a.txt"), "v1").expect("versioned_simulate add/delete write a v1");
    let cfg = build_cfg(&source, &dest);
    run_backup_cycle(&cfg).expect("versioned_simulate add/delete initial backup");

    let b = source.join("b.txt");
    fs::write(&b, "hello").expect("versioned_simulate add/delete write b");
    let sim_add = simulate_backup_cycle_with_sample_limit(&cfg, 1)
        .expect("versioned_simulate add/delete simulate add");
    assert_eq!(sim_add.adds, 1);
    assert_eq!(sim_add.deletes, 0);
    assert_eq!(sim_add.items, 1);
    assert_eq!(sim_add.sample.len(), 1);
    assert!(
        sim_add.sample[0].starts_with("+ "),
        "expected an add sample entry"
    );

    run_backup_cycle(&cfg).expect("versioned_simulate add/delete commit add");
    fs::remove_file(&b).expect("versioned_simulate add/delete remove b");
    let sim_del = simulate_backup_cycle_with_sample_limit(&cfg, 2)
        .expect("versioned_simulate add/delete simulate delete");
    assert_eq!(sim_del.adds, 0);
    assert_eq!(sim_del.deletes, 1);
    assert_eq!(sim_del.items, 1);
    assert_eq!(sim_del.bytes_to_write, 0);
    assert!(
        sim_del.sample.iter().any(|s| s.starts_with("- ")),
        "expected a delete sample entry"
    );
}
