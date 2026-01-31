use super::logging;
use anyhow::{Context, Result};
use backup_core::{
    backup::{execution::BackupExecutor, planning},
    fs::{
        scanning::collect_targets,
        watching::{debounce::debounce_and_take, debounce_duration, DirtySet},
    },
    verify_backups, Config, HashingTuning, StateStore, StoredState,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

/// Purpose: Runs a full backup cycle including planning, safe mode checks, and execution.
///
/// Inputs: config, dirty set, state store, and shared state.
/// Outputs: `Ok(())` when the cycle completes without fatal errors.
/// Ties to: the daemon scheduler loop.
/// Side effects: Reads filesystem metadata, performs backup IO, mutates state, and emits logs.
/// Why: encapsulate the end to end backup cycle behavior.
pub async fn run_cycle(
    cfg: &Config,
    dirty: &DirtySet,
    store: &StateStore,
    shared_state: &Arc<Mutex<StoredState>>,
) -> Result<()> {
    let cycle_cid = logging::cid("cycle");
    logging::log_info("run_cycle_start", &cycle_cid, None, "starting backup cycle");
    let mut state = shared_state.lock().await.clone();
    prune_missing(&mut state, cfg.runtime.prune_interval_cycles);
    let filtered = collect_filtered_targets(cfg, dirty, &mut state)
        .await
        .with_context(|| "daemon::runtime::run_cycle failed to collect filtered targets")?;
    let plan = planning::plan(filtered, &mut state, &cfg.planning, &cfg.hashing)
        .with_context(|| "daemon::runtime::run_cycle failed to build plan")?;
    planning::enforce_plan_limits(&plan, &cfg.planning)
        .with_context(|| "daemon::runtime::run_cycle plan limit exceeded")?;
    if plan.is_empty() {
        logging::log_info(
            "cycle_no_changes",
            &cycle_cid,
            None,
            "no changes detected; skipping backup",
        );
        return Ok(());
    }
    if apply_safe_mode(cfg, &plan, &mut state, store, &cycle_cid, shared_state)
        .await
        .with_context(|| "daemon::runtime::run_cycle failed during safe mode check")?
    {
        return Ok(());
    }
    execute_and_persist(cfg, &plan, &mut state, store, &cycle_cid, shared_state)
        .await
        .with_context(|| "daemon::runtime::run_cycle failed during execution")?;
    Ok(())
}

/// Purpose: Removes missing file entries from state on a periodic cadence.
///
/// Inputs: mutable state containing tracked files and the prune interval in cycles.
/// Outputs: `()` after pruning missing entries.
/// Ties to: state maintenance during cycle execution.
/// Side effects: Reads filesystem metadata and mutates stored state.
/// Why: keep state aligned with on disk reality.
fn prune_missing(state: &mut StoredState, prune_interval_cycles: u64) {
    state.prune_counter = state.prune_counter.saturating_add(1);
    if state.prune_counter % prune_interval_cycles == 0 {
        let removed: Vec<_> = state
            .files
            .iter()
            .filter_map(|(k, _)| {
                let p = std::path::Path::new(k);
                if !p.exists() {
                    Some(k.clone())
                } else {
                    None
                }
            })
            .collect();
        for k in &removed {
            state.files.remove(k);
        }
        if !removed.is_empty() {
            info!("pruned {} missing files from state", removed.len());
        }
    }
}

/// Purpose: Collects scan targets and filters them using the dirty set when available.
///
/// Inputs: config, dirty set, and mutable state for metrics.
/// Outputs: the filtered list of file metadata.
/// Ties to: scan collection, watcher debounce, and state tracking.
/// Side effects: Reads filesystem metadata and drains the dirty set.
/// Why: avoid scanning everything when a small dirty set is available.
async fn collect_filtered_targets(
    cfg: &Config,
    dirty: &DirtySet,
    state: &mut StoredState,
) -> Result<Vec<backup_core::fs::scanning::metadata::FileMeta>> {
    let targets = collect_targets(&cfg)
        .context("daemon::runtime::collect_filtered_targets failed to collect targets")?;
    let dirty_paths = debounce_and_take(dirty, debounce_duration(&cfg.runtime)).await;
    state.last_dirty_count = dirty_paths.len();
    if dirty_paths.is_empty() {
        Ok(targets)
    } else {
        Ok(targets
            .into_iter()
            .filter(|m| dirty_paths.iter().any(|p| p == &m.path))
            .collect())
    }
}

/// Purpose: Applies safe mode behavior and persists state when writes are skipped.
///
/// Inputs: config, plan, state, store, correlation id, and shared state handle.
/// Outputs: `Ok(true)` when safe mode short-circuits the cycle.
/// Ties to: safe mode enforcement and state persistence.
/// Side effects: Mutates stored state and persists it when safe mode is active.
/// Why: prevent writes while still updating run metadata.
pub(crate) async fn apply_safe_mode(
    cfg: &Config,
    plan: &[planning::PlannedItem],
    state: &mut StoredState,
    store: &StateStore,
    cid: &str,
    shared_state: &Arc<Mutex<StoredState>>,
) -> Result<bool> {
    if cfg.safe_mode || state.safe_mode {
        logging::log_info(
            "safe_mode_skip",
            cid,
            None,
            &format!("skipping writes. {} items would be backed up.", plan.len()),
        );
        state.last_run_ts = Some(chrono::Utc::now().timestamp());
        state.last_files_backed_up = 0;
        state.last_error = Some("Safe mode: no writes performed".into());
        store
            .persist(&state)
            .context("daemon::runtime::apply_safe_mode failed to persist state in safe mode")?;
        *shared_state.lock().await = state.clone();
        return Ok(true);
    }
    Ok(false)
}

/// Purpose: Executes the plan and persists resulting state updates.
///
/// Inputs: config, plan, mutable state, store, correlation id, and shared state handle.
/// Outputs: `Ok(())` when execution and persistence complete.
/// Ties to: backup execution and state persistence.
/// Side effects: Performs backup IO, writes state to disk, and emits logs.
/// Why: centralize execution side effects and state updates.
async fn execute_and_persist(
    cfg: &Config,
    plan: &[planning::PlannedItem],
    state: &mut StoredState,
    store: &StateStore,
    cid: &str,
    shared_state: &Arc<Mutex<StoredState>>,
) -> Result<()> {
    let executor = BackupExecutor::from_config(cfg);
    let result = executor
        .execute(&plan, state)
        .context("daemon::runtime::execute_and_persist backup execution failed")?;
    state.last_files_backed_up = result.backed_up;
    state.last_run_ts = Some(chrono::Utc::now().timestamp());
    store
        .persist(&state)
        .context("daemon::runtime::execute_and_persist failed to persist state after backup")?;
    logging::log_info(
        "backup_complete",
        cid,
        Some(std::path::Path::new(&cfg.backup_root)),
        &format!("items={} errors={}", result.backed_up, result.errors),
    );
    *shared_state.lock().await = state.clone();
    Ok(())
}

/// Purpose: Runs a verification cycle and persists verification results.
///
/// Inputs: the state store and shared state handle.
/// Outputs: `Ok(())` after persisting verification updates.
/// Ties to: scheduled verification flows.
/// Side effects: Reads backup files for hashing, mutates state, and writes state to disk.
/// Why: keep verification results up to date for UI and logs.
pub async fn run_verify_cycle(
    store: &StateStore,
    shared_state: &Arc<Mutex<StoredState>>,
    hashing: &HashingTuning,
) -> Result<()> {
    let cid = logging::cid("verify");
    let mut guard = shared_state.lock().await;
    match verify_backups(&mut guard, hashing) {
        Ok((ok, bad)) => {
            logging::log_info(
                "verify_complete",
                &cid,
                None,
                &format!("ok={} issues={}", ok, bad),
            );
        }
        Err(e) => {
            guard.last_verify_ts = Some(chrono::Utc::now().timestamp());
            guard.last_verify_status = Some(format!("failed: {e}"));
            guard.last_verify_issues = None;
            logging::log_warn("verify_failed", &cid, None, &format!("verify error: {e:?}"));
        }
    }
    store
        .persist(&guard)
        .context("daemon::runtime::run_verify_cycle failed to persist state after verify")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// Purpose: Runs apply_safe_mode with a configured safe_mode setting and returns results.
    ///
    /// Inputs: the desired safe_mode flag.
    /// Outputs: the resulting state and skip flag.
    /// Ties to: safe mode behavior tests.
    /// Side effects: None.
    /// Why: keep test setup for safe mode behavior consistent.
    async fn apply_safe_mode_case(safe_mode: bool) -> (StoredState, bool) {
        let _tmp = tempdir().expect("cycle::apply_safe_mode_case failed to create temp dir");
        let root = _tmp.path().to_path_buf();
        let cfg = Config {
            backup_root: root.clone(),
            interval_seconds: 5,
            max_backups_per_file: 1,
            skip_hidden: true,
            ignore_patterns: vec![],
            max_parallel_copies: 1,
            max_bytes_per_second: None,
            min_free_space_bytes: None,
            hashing: backup_core::config::model::HashingTuning::default(),
            execution: backup_core::config::model::ExecutionTuning::default(),
            planning: backup_core::config::model::PlanningTuning::default(),
            runtime: backup_core::config::model::RuntimeTuning::default(),
            watched: vec![],
            safe_mode,
            destinations: vec![backup_core::config::model::Destination {
                id: "default".into(),
                path: root.clone(),
                label: None,
                max_backups_per_file: None,
            }],
        };
        let store_path =
            tempdir().expect("cycle::apply_safe_mode_case failed to create store temp dir");
        let store = StateStore::load_or_default(store_path.path().join("state.json"))
            .expect("cycle::apply_safe_mode_case failed to load state store")
            .1;
        let shared = Arc::new(Mutex::new(StoredState::default()));
        let mut state = StoredState::default();
        let plan: Vec<planning::PlannedItem> = vec![];
        let cid = "test-safe-mode";
        let skipped = apply_safe_mode(&cfg, &plan, &mut state, &store, cid, &shared)
            .await
            .expect("cycle::apply_safe_mode_case failed to apply safe mode");
        (state, skipped)
    }

    #[tokio::test]
    /// Purpose: Ensures safe mode short-circuits execution and updates state.
    ///
    /// Inputs: a safe_mode enabled config.
    /// Outputs: a skipped flag and updated state fields.
    /// Ties to: safe mode enforcement.
    /// Side effects: None.
    /// Why: avoid writes while reporting safe mode status.
    async fn safe_mode_short_circuits_and_sets_state() {
        let (state, skipped) = apply_safe_mode_case(true).await;
        assert!(skipped, "safe mode should short-circuit execution");
        assert_eq!(state.last_files_backed_up, 0);
        assert!(state.last_error.unwrap_or_default().contains("Safe mode"));
    }

    #[tokio::test]
    /// Purpose: Ensures normal execution continues when safe mode is disabled.
    ///
    /// Inputs: a safe_mode disabled config.
    /// Outputs: a skip flag set to false.
    /// Ties to: safe mode enforcement.
    /// Side effects: None.
    /// Why: allow backups when safe mode is off.
    async fn safe_mode_disabled_continues() {
        let (_state, skipped) = apply_safe_mode_case(false).await;
        assert!(!skipped, "should continue when safe mode disabled");
    }
}
