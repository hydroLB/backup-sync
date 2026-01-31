use backup_core::{
    backup::{execution::BackupExecutor, planning},
    config::model::{
        Config, ExecutionTuning, HashingTuning, PlanningTuning, RuntimeTuning, WatchedKind,
        WatchedPath,
    },
    fs::scanning::collect_targets,
    state::store::StateStore,
    verify_backups,
};
use tempfile::tempdir;

#[test]
/// Purpose: Runs a full scan, plan, execute, and verify cycle as a smoke test.
///
/// Inputs: a temporary watched directory and destination.
/// Outputs: a successful backup and verification result.
/// Ties to: the end to end workflow for backups.
/// Side effects: None.
/// Why: ensure the primary workflow stays healthy across refactors.
fn e2e_smoke_backup_cycle() {
    let dir = tempdir().expect("e2e_smoke::e2e_smoke_backup_cycle failed to create temp dir");
    let watched_dir = dir.path().join("watched");
    let backup_root = dir.path().join("backups");
    std::fs::create_dir_all(&watched_dir)
        .expect("e2e_smoke::e2e_smoke_backup_cycle failed to create watched dir");
    std::fs::create_dir_all(&backup_root)
        .expect("e2e_smoke::e2e_smoke_backup_cycle failed to create backup root");
    let file_path = watched_dir.join("note.txt");
    std::fs::write(&file_path, "hello world")
        .expect("e2e_smoke::e2e_smoke_backup_cycle failed to write watched file");

    let cfg = Config {
        backup_root: backup_root.clone(),
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
        runtime: RuntimeTuning::default(),
        safe_mode: false,
        watched: vec![WatchedPath {
            path: watched_dir,
            kind: WatchedKind::Directory,
            enabled: true,
            destination_id: "default".into(),
            max_backups_per_file: None,
        }],
        destinations: vec![backup_core::config::model::Destination {
            id: "default".into(),
            path: backup_root,
            label: None,
            max_backups_per_file: None,
        }],
    };

    let state_path = dir.path().join("state.json");
    let (mut state, store) = StateStore::load_or_default(state_path)
        .expect("e2e_smoke::e2e_smoke_backup_cycle failed to load state");
    let targets =
        collect_targets(&cfg).expect("e2e_smoke::e2e_smoke_backup_cycle failed to collect targets");
    let plan = planning::plan(targets, &mut state, &cfg.planning, &cfg.hashing)
        .expect("e2e_smoke::e2e_smoke_backup_cycle failed to build plan");
    assert_eq!(plan.len(), 1, "expected one planned item");
    let exec = BackupExecutor::from_config(&cfg);
    let res = exec
        .execute(&plan, &mut state)
        .expect("e2e_smoke::e2e_smoke_backup_cycle failed to execute plan");
    assert_eq!(res.backed_up, 1);
    store
        .persist(&state)
        .expect("e2e_smoke::e2e_smoke_backup_cycle failed to persist state");
    let (ok, bad) = verify_backups(&mut state, &cfg.hashing)
        .expect("e2e_smoke::e2e_smoke_backup_cycle failed to verify backups");
    assert!(ok >= 1);
    assert_eq!(bad, 0);
}
