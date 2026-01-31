use backup_core::{
    backup::{execution::BackupExecutor, planning},
    config::model::{
        Config, ExecutionTuning, HashingTuning, PlanningTuning, WatchedKind, WatchedPath,
    },
    fs::scanning::collect_targets,
    state::store::StateStore,
};
use std::fs;
use tempfile::tempdir;

#[test]
/// Purpose: Ensures unchanged files are not re-backed up after state reload.
///
/// Inputs: a stable file, persisted state, and a reloaded plan.
/// Outputs: an empty plan on the second run.
/// Ties to: planner stability checks and stored state reuse.
/// Side effects: None.
/// Why: avoid redundant backups when nothing has changed.
fn no_rebackup_when_state_reloaded_and_unchanged() {
    let dir = tempdir().expect(
        "no_rebackup_hash::no_rebackup_when_state_reloaded_and_unchanged failed to create temp dir",
    );
    let file = dir.path().join("c.txt");
    fs::write(&file, "hello").expect(
        "no_rebackup_hash::no_rebackup_when_state_reloaded_and_unchanged failed to write file",
    );
    let cfg = Config {
        backup_root: dir.path().join("backups"),
        interval_seconds: 60,
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
    let state_path = dir.path().join("state.json");
    let (mut state, store) = StateStore::load_or_default(state_path.clone()).expect(
        "no_rebackup_hash::no_rebackup_when_state_reloaded_and_unchanged failed to load state",
    );

    let targets = collect_targets(&cfg).expect(
        "no_rebackup_hash::no_rebackup_when_state_reloaded_and_unchanged failed to collect targets",
    );
    let plan = planning::plan(targets, &mut state, &cfg.planning, &cfg.hashing).expect(
        "no_rebackup_hash::no_rebackup_when_state_reloaded_and_unchanged failed to build plan",
    );
    let exec = BackupExecutor::from_config(&cfg);
    exec.execute(&plan, &mut state).expect(
        "no_rebackup_hash::no_rebackup_when_state_reloaded_and_unchanged failed to execute plan",
    );
    store.persist(&state).expect(
        "no_rebackup_hash::no_rebackup_when_state_reloaded_and_unchanged failed to persist state",
    );

    // Reload state and ensure no new backup if unchanged
    let (mut state2, _store2) = StateStore::load_or_default(state_path.clone()).expect(
        "no_rebackup_hash::no_rebackup_when_state_reloaded_and_unchanged failed to reload state",
    );
    let targets2 = collect_targets(&cfg).expect(
        "no_rebackup_hash::no_rebackup_when_state_reloaded_and_unchanged failed to collect targets on reload",
    );
    let plan2 = planning::plan(targets2, &mut state2, &cfg.planning, &cfg.hashing).expect(
        "no_rebackup_hash::no_rebackup_when_state_reloaded_and_unchanged failed to build plan on reload",
    );
    assert!(
        plan2.is_empty(),
        "no backup should be planned when unchanged"
    );
}
