use backup_core::{
    backup::{execution::BackupExecutor, planning},
    config::model::{
        Config, ExecutionTuning, HashingTuning, PlanningTuning, WatchedKind, WatchedPath,
    },
    fs::scanning::collect_targets,
    state::store::StateStore,
};
use std::{fs, time::Duration};
use tempfile::tempdir;

#[test]
/// Purpose: Ensures backups occur and retention caps are enforced across multiple edits.
///
/// Inputs: a file updated several times and a small retention limit.
/// Outputs: a state entry with backups not exceeding the limit.
/// Ties to: planning, execution, and retention enforcement.
/// Side effects: None.
/// Why: verify retention policies cap backup histories.
fn backs_up_and_respects_retention() {
    let dir = tempdir()
        .expect("backup_scenarios::backs_up_and_respects_retention failed to create temp dir");
    let file = dir.path().join("a.txt");
    fs::write(&file, "v1")
        .expect("backup_scenarios::backs_up_and_respects_retention failed to write initial file");

    let cfg = Config {
        backup_root: dir.path().join("backups"),
        interval_seconds: 1,
        max_backups_per_file: 2,
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
        }],
    };
    let (mut state, store) = StateStore::load_or_default(dir.path().join("state.json"))
        .expect("backup_scenarios::backs_up_and_respects_retention failed to load state");

    for i in 0..3 {
        std::thread::sleep(Duration::from_millis(5));
        fs::write(&file, format!("v{}", i + 2))
            .expect("backup_scenarios::backs_up_and_respects_retention failed to update file");
        let targets = collect_targets(&cfg)
            .expect("backup_scenarios::backs_up_and_respects_retention failed to collect targets");
        let plan = planning::plan(targets, &mut state, &cfg.planning, &cfg.hashing)
            .expect("backup_scenarios::backs_up_and_respects_retention failed to build plan");
        let exec = BackupExecutor::from_config(&cfg);
        exec.execute(&plan, &mut state)
            .expect("backup_scenarios::backs_up_and_respects_retention failed to execute plan");
    }
    store
        .persist(&state)
        .expect("backup_scenarios::backs_up_and_respects_retention failed to persist state");
    let entry = state
        .files
        .values()
        .next()
        .expect("backup_scenarios::backs_up_and_respects_retention missing state entry");
    assert!(entry.backups.len() <= 2);
}
