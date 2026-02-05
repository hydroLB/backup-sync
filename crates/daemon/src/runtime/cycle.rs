use super::logging;
use anyhow::{Context, Result};
use backup_core::{
    backup::versioned,
    fs::watching::{debounce::debounce_and_take, debounce_duration, DirtySet},
    Config, HashingTuning, StateStore, StoredState,
};
use std::sync::Arc;
use tokio::sync::Mutex;

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
    state.last_dirty_count = drain_dirty_count(cfg, dirty).await;
    if apply_safe_mode(cfg, &mut state, store, &cycle_cid, shared_state)
        .await
        .with_context(|| "daemon::runtime::run_cycle failed during safe mode check")?
    {
        return Ok(());
    }

    let result = versioned::run_backup_cycle(cfg)
        .with_context(|| "daemon::runtime::run_cycle versioned backup cycle failed")?;
    state.last_files_backed_up = result.versions_created;
    state.last_run_ts = Some(chrono::Utc::now().timestamp());
    state.last_error = None;
    store
        .persist(&state)
        .context("daemon::runtime::run_cycle failed to persist state after backup")?;
    *shared_state.lock().await = state.clone();

    if result.versions_created == 0 {
        logging::log_info(
            "cycle_no_changes",
            &cycle_cid,
            None,
            "no changes detected; skipping backup",
        );
    } else {
        logging::log_info(
            "backup_complete",
            &cycle_cid,
            Some(std::path::Path::new(&cfg.backup_root)),
            &format!(
                "folders={} versions={} blobs={} bytes={}",
                result.folders_scanned,
                result.versions_created,
                result.blobs_written,
                result.bytes_written
            ),
        );
    }
    Ok(())
}

/// Purpose: Drains the dirty set and returns a debounced count for metrics.
///
/// Inputs: config and dirty set.
/// Outputs: count of dirty paths drained since last call.
/// Side effects: Drains the dirty set.
/// Error handling: Never fails; returns 0 on errors.
/// Ties to other methods: Used by `run_cycle` for UI observability.
/// Why this exists: Preserve watcher metrics while decoupling from backup engine internals.
async fn drain_dirty_count(cfg: &Config, dirty: &DirtySet) -> usize {
    let dirty_paths = debounce_and_take(dirty, debounce_duration(&cfg.runtime)).await;
    dirty_paths.len()
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
            "skipping writes; safe mode active",
        );
        state.last_run_ts = Some(chrono::Utc::now().timestamp());
        state.last_files_backed_up = 0;
        state.last_error = Some("Safe mode: no writes performed".into());
        store
            .persist(state)
            .context("daemon::runtime::apply_safe_mode failed to persist state in safe mode")?;
        *shared_state.lock().await = state.clone();
        return Ok(true);
    }
    Ok(false)
}

/// Purpose: Runs a verification cycle and persists verification results.
///
/// Inputs: the state store and shared state handle.
/// Outputs: `Ok(())` after persisting verification updates.
/// Ties to: scheduled verification flows.
/// Side effects: Reads backup files for hashing, mutates state, and writes state to disk.
/// Why: keep verification results up to date for UI and logs.
pub async fn run_verify_cycle(
    cfg: &Config,
    store: &StateStore,
    shared_state: &Arc<Mutex<StoredState>>,
    hashing: &HashingTuning,
) -> Result<()> {
    let cid = logging::cid("verify");
    let mut guard = shared_state.lock().await;
    let now = chrono::Utc::now().timestamp();
    let full_due = match guard.last_scrub_full_ts {
        None => true,
        Some(ts) => now.saturating_sub(ts) >= cfg.runtime.scrub_full_interval_seconds as i64,
    };
    let mode = if full_due {
        versioned::ScrubMode::Full
    } else {
        versioned::ScrubMode::Sampled
    };
    match versioned::scrub_versioned_store(
        cfg,
        hashing,
        mode,
        cfg.runtime.scrub_sample_blobs,
        cfg.runtime.scrub_sample_versions_per_source,
        now as u64,
    ) {
        Ok(res) => {
            let bad = res.hash_mismatches + res.missing_blobs + res.manifests_bad;
            let ok = res.blobs_hashed.saturating_sub(res.hash_mismatches);
            let mode_label = match res.mode {
                versioned::ScrubMode::Sampled => "sampled",
                versioned::ScrubMode::Full => "full",
            };
            guard.last_verify_ts = Some(now);
            guard.last_verify_issues = Some(bad);
            guard.last_verify_status = Some(if bad == 0 {
                format!("ok ({mode_label})")
            } else {
                format!("issues_detected ({mode_label})")
            });
            if res.mode == versioned::ScrubMode::Full {
                guard.last_scrub_full_ts = Some(now);
            }
            logging::log_info(
                "verify_complete",
                &cid,
                None,
                &format!(
                    "mode={mode_label} manifests_checked={} manifests_bad={} referenced_blobs={} hashed={} ok={} missing={} mismatches={}",
                    res.manifests_checked,
                    res.manifests_bad,
                    res.referenced_blobs,
                    res.blobs_hashed,
                    ok,
                    res.missing_blobs,
                    res.hash_mismatches
                ),
            );
        }
        Err(e) => {
            guard.last_verify_ts = Some(now);
            guard.last_verify_status = Some(format!("failed: {e}"));
            guard.last_verify_issues = None;
            logging::log_warn("verify_failed", &cid, None, &format!("verify error: {e:?}"));
        }
    };
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
        let cid = "test-safe-mode";
        let skipped = apply_safe_mode(&cfg, &mut state, &store, cid, &shared)
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
