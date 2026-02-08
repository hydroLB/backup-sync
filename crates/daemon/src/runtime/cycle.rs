use super::logging;
use anyhow::{Context, Result};
use backup_core::{
    backup::versioned,
    fs::watching::{debounce::debounce_and_take, debounce_duration, DirtySet},
    Config, HashingTuning, StateStore, StoredState,
};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
struct DestinationWriteHealth {
    id: String,
    ok: bool,
    message: String,
}

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
    state.last_dirty_count = dirty.0.lock().len();

    let now = chrono::Utc::now().timestamp();
    let needed_destination_ids = required_destination_ids(cfg);
    let dest_health = probe_required_destinations_for_write(cfg, &needed_destination_ids);
    let unavailable: Vec<String> = dest_health
        .iter()
        .filter(|d| !d.ok)
        .map(|d| d.id.clone())
        .collect();
    state.destination_unavailable_ids = unavailable.clone();

    let pause_reason = if unavailable.is_empty() {
        None
    } else {
        let details: Vec<String> = dest_health
            .iter()
            .filter(|d| !d.ok)
            .map(|d| format!("{} ({})", d.id, d.message))
            .collect();
        Some(format!(
            "Destination unavailable: {}. Writes paused and will auto-resume when the destination returns.",
            details.join(", ")
        ))
    };
    let was_paused = state.destination_paused;
    let is_paused = pause_reason.is_some();
    state.destination_paused = is_paused;
    state.destination_pause_reason = pause_reason.clone();
    if is_paused && (!was_paused || state.destination_last_unavailable_ts.is_none()) {
        state.destination_last_unavailable_ts = Some(now);
    }
    let destination_recovered = was_paused && !is_paused;
    if destination_recovered {
        state.destination_last_recovered_ts = Some(now);
        state.destination_unavailable_ids.clear();
        // Clear the destination pause error if it was the last error recorded.
        if state
            .last_error
            .as_deref()
            .unwrap_or_default()
            .starts_with("Destination unavailable:")
        {
            state.last_error = None;
        }
        logging::log_info(
            "destination_recovered",
            &cycle_cid,
            None,
            "destination recovered; resuming backup writes",
        );
    }

    if let Some(reason) = pause_reason.as_deref() {
        logging::log_warn("destination_unavailable", &cycle_cid, None, reason);
        if apply_safe_mode(cfg, &mut state, store, &cycle_cid, shared_state)
            .await
            .with_context(|| "daemon::runtime::run_cycle failed during safe mode check")?
        {
            return Ok(());
        }
        state.last_run_ts = Some(now);
        state.last_files_backed_up = 0;
        state.cycles_since_full_scan = state.cycles_since_full_scan.saturating_add(1);
        state.last_error = Some(reason.to_string());
        store.persist(&state).context(
            "daemon::runtime::run_cycle failed to persist state after destination pause",
        )?;
        *shared_state.lock().await = state.clone();
        return Ok(());
    }

    if apply_safe_mode(cfg, &mut state, store, &cycle_cid, shared_state)
        .await
        .with_context(|| "daemon::runtime::run_cycle failed during safe mode check")?
    {
        return Ok(());
    }

    state.last_dirty_count = drain_dirty_count(cfg, dirty).await;
    match decide_scan(state.last_dirty_count, &state, cfg, destination_recovered) {
        ScanDecision::SkipClean { force_due_in } => {
            state.cycles_since_full_scan = state.cycles_since_full_scan.saturating_add(1);
            state.last_run_ts = Some(now);
            state.last_files_backed_up = 0;
            state.last_error = None;
            store
                .persist(&state)
                .context("daemon::runtime::run_cycle failed to persist state after clean skip")?;
            *shared_state.lock().await = state.clone();
            logging::log_info(
                "cycle_clean_skip",
                &cycle_cid,
                None,
                &format!(
                    "dirty=0; skipping full scan (force_due_in_cycles={})",
                    force_due_in
                ),
            );
            return Ok(());
        }
        ScanDecision::RunFullScan { reason } => {
            logging::log_info("cycle_full_scan", &cycle_cid, None, reason);
        }
    }

    let result = versioned::run_backup_cycle(cfg)
        .with_context(|| "daemon::runtime::run_cycle versioned backup cycle failed")?;
    state.last_files_backed_up = result.versions_created;
    state.last_run_ts = Some(now);
    state.last_error = None;
    state.cycles_since_full_scan = 0;
    if let Some(w) = result.safety_warnings.last().cloned() {
        state.last_safety_warning = Some(w);
    }

    let has_replication_pairs = cfg.destinations.iter().any(|d| !d.replicate_to.is_empty());
    if has_replication_pairs && cfg.runtime.replication_enabled {
        match versioned::replicate_configured_stores(cfg) {
            Ok(rep) => {
                state.replication_last_run_ts = Some(now);
                state.replication_last_bytes_copied = rep.bytes_copied;
                state.replication_last_blobs_copied = rep.blobs_copied;
                state.replication_last_manifests_copied = rep.manifests_copied;
                state.replication_last_manifests_deleted = rep.manifests_deleted;
                state.replication_last_pairs_ok = rep.pairs_ok;
                state.replication_last_pairs_failed = rep.pairs_failed;
                state.replication_last_targets_failed = rep.targets_failed.clone();
                state.replication_last_status = Some(if rep.pairs_failed == 0 {
                    "ok".to_string()
                } else {
                    "degraded".to_string()
                });
                state.replication_last_error = if rep.pairs_failed == 0 {
                    None
                } else {
                    Some(rep.message.clone())
                };
                logging::log_info("replication_complete", &cycle_cid, None, &rep.message);
            }
            Err(e) => {
                state.replication_last_run_ts = Some(now);
                state.replication_last_status = Some("failed".to_string());
                state.replication_last_error = Some(format!("{e:#}"));
                logging::log_warn(
                    "replication_failed",
                    &cycle_cid,
                    None,
                    &format!("replication error: {e:#}"),
                );
            }
        }
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanDecision<'a> {
    RunFullScan { reason: &'a str },
    SkipClean { force_due_in: u64 },
}

/// Purpose: Decide whether the daemon should run a full versioned scan this cycle.
///
/// Inputs: watcher dirty count, current stored state, and config runtime tuning.
/// Outputs: a `ScanDecision` indicating whether to run the scan or skip it.
/// Ties to: `run_cycle` and watcher-based scan skipping behavior.
/// Side effects: None.
/// Why: watchers can often detect changes; skipping clean cycles avoids expensive full scans while still forcing periodic safety scans.
fn decide_scan<'a>(
    dirty_count: usize,
    state: &StoredState,
    cfg: &'a Config,
    force_full_scan: bool,
) -> ScanDecision<'a> {
    if force_full_scan {
        return ScanDecision::RunFullScan {
            reason: "destination recovered; running full scan",
        };
    }
    if dirty_count > 0 {
        return ScanDecision::RunFullScan {
            reason: "dirty paths present; running full scan",
        };
    }

    // Ensure the first cycle after daemon start runs a full scan. Watchers do not run while the
    // daemon is stopped, so relying solely on the dirty set could delay backups after restarts.
    let is_first_cycle_after_start = match (state.start_ts, state.last_run_ts) {
        (None, None) => true,
        (Some(_), None) => true,
        (Some(start), Some(last)) => last < start,
        (None, Some(_)) => false,
    };
    if is_first_cycle_after_start {
        return ScanDecision::RunFullScan {
            reason: "first cycle after daemon start; running full scan",
        };
    }
    let interval = cfg.runtime.force_full_scan_interval_cycles.max(1);
    if interval <= 1 {
        return ScanDecision::RunFullScan {
            reason: "force_full_scan_interval_cycles=1; running full scan",
        };
    }
    if state.cycles_since_full_scan >= interval.saturating_sub(1) {
        return ScanDecision::RunFullScan {
            reason: "forced full scan due; running safety scan",
        };
    }
    let due_in = interval
        .saturating_sub(1)
        .saturating_sub(state.cycles_since_full_scan);
    ScanDecision::SkipClean {
        force_due_in: due_in,
    }
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
    if dirty.0.lock().is_empty() {
        return 0;
    }
    let dirty_paths = debounce_and_take(dirty, debounce_duration(&cfg.runtime)).await;
    dirty_paths.len()
}

/// Purpose: Return the set of destination ids required by enabled watched paths.
///
/// Inputs: loaded config.
/// Outputs: a set of destination ids referenced by enabled watched entries.
/// Side effects: None.
/// Error handling: Never fails; returns an empty set when nothing is watched.
/// Ties to other methods: Used by destination health checks and write pause decisions.
/// Why this exists: The daemon should only pause writes for destinations that are actually in use.
fn required_destination_ids(cfg: &Config) -> HashSet<&str> {
    cfg.watched
        .iter()
        .filter(|w| w.enabled)
        .map(|w| w.destination_id.as_str())
        .collect()
}

/// Purpose: Probe destinations required by watched paths to decide whether writes can proceed.
///
/// Inputs: config and the set of required destination ids.
/// Outputs: a vector of per destination health results.
/// Side effects: May create directories under each destination to validate write readiness.
/// Error handling: Never panics; returns `ok=false` with contextual message when probing fails.
/// Ties to other methods: Used by `run_cycle` to pause writes when a destination is disconnected.
/// Why this exists: Detecting disconnected or unwritable destinations early avoids spamming engine errors and enables auto-resume.
fn probe_required_destinations_for_write(
    cfg: &Config,
    needed_ids: &HashSet<&str>,
) -> Vec<DestinationWriteHealth> {
    let by_id: HashMap<&str, &backup_core::config::model::Destination> = cfg
        .destinations
        .iter()
        .map(|d| (d.id.as_str(), d))
        .collect();
    needed_ids
        .iter()
        .filter_map(|id| by_id.get(id).copied())
        .map(|d| probe_destination_for_write(d))
        .collect()
}

/// Purpose: Probe a single destination for “write-ready” status.
///
/// Inputs: destination configuration.
/// Outputs: `DestinationWriteHealth` with `ok=true` when the destination can be written to.
/// Side effects: May create `.backup_sync` folder under the destination path.
/// Error handling: Returns a non-OK probe with a clear message on IO failures.
/// Ties to other methods: Used by `probe_required_destinations_for_write` and `run_cycle`.
/// Why this exists: Create-dir failures are the most common symptom of disconnected drives or permission issues.
fn probe_destination_for_write(
    d: &backup_core::config::model::Destination,
) -> DestinationWriteHealth {
    let root: &Path = d.path.as_path();
    if root.as_os_str().is_empty() {
        return DestinationWriteHealth {
            id: d.id.clone(),
            ok: false,
            message: "destination path is empty".to_string(),
        };
    }
    if !root.exists() {
        return DestinationWriteHealth {
            id: d.id.clone(),
            ok: false,
            message: "destination folder not found (drive disconnected?)".to_string(),
        };
    }
    if root.is_file() {
        return DestinationWriteHealth {
            id: d.id.clone(),
            ok: false,
            message: "destination path is a file".to_string(),
        };
    }
    if !root.is_dir() {
        return DestinationWriteHealth {
            id: d.id.clone(),
            ok: false,
            message: "destination path is not a directory".to_string(),
        };
    }

    let probe_dir = root.join(".backup_sync");
    if let Err(e) = fs::create_dir_all(&probe_dir) {
        return DestinationWriteHealth {
            id: d.id.clone(),
            ok: false,
            message: format!("cannot create store directory: {e}"),
        };
    }
    DestinationWriteHealth {
        id: d.id.clone(),
        ok: true,
        message: "ok".to_string(),
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
        state.cycles_since_full_scan = state.cycles_since_full_scan.saturating_add(1);
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

    let needed_destination_ids = required_destination_ids(cfg);
    let unavailable: Vec<_> = cfg
        .destinations
        .iter()
        .filter(|d| needed_destination_ids.contains(d.id.as_str()))
        .filter(|d| !d.path.exists() || !d.path.is_dir())
        .map(|d| d.id.clone())
        .collect();
    if !unavailable.is_empty() {
        let msg = format!(
            "skipped: destination unavailable ({})",
            unavailable.join(", ")
        );
        guard.last_verify_ts = Some(now);
        guard.last_verify_status = Some(msg.clone());
        guard.last_verify_issues = None;
        logging::log_warn("verify_skipped", &cid, None, &msg);
        store.persist(&guard).context(
            "daemon::runtime::run_verify_cycle failed to persist state after verify skip",
        )?;
        return Ok(());
    }

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

    #[test]
    /// Purpose: Ensures the scan decision runs on first cycle even when no dirty paths exist.
    ///
    /// Inputs: A default stored state with no `last_run_ts`.
    /// Outputs: A `RunFullScan` decision.
    /// Ties to: startup cycle behavior.
    /// Side effects: None.
    /// Why: the first cycle must establish baseline state even if watchers have not fired yet.
    fn decide_scan_runs_on_first_cycle() {
        let mut cfg = Config {
            backup_root: std::path::PathBuf::from("/tmp"),
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
            encryption: backup_core::config::model::EncryptionConfig::default(),
            compression: backup_core::config::model::CompressionConfig::default(),
            watched: vec![],
            safe_mode: false,
            destinations: vec![],
        };
        cfg.runtime.force_full_scan_interval_cycles = 24;
        let mut state = StoredState::default();
        state.start_ts = Some(10);
        state.last_run_ts = Some(1);
        let decision = decide_scan(0, &state, &cfg, false);
        assert!(
            matches!(decision, ScanDecision::RunFullScan { .. }),
            "expected full scan decision on first cycle"
        );
    }

    #[test]
    /// Purpose: Ensures clean cycles are skipped until the forced full-scan interval is due.
    ///
    /// Inputs: A stored state with a prior run, no dirty paths, and a configured interval.
    /// Outputs: Skip decision before due and run decision when due.
    /// Ties to: watcher-based scan skipping.
    /// Side effects: None.
    /// Why: avoid expensive scans when nothing changed while still providing a periodic safety scan.
    fn decide_scan_skips_clean_until_due() {
        let mut cfg = Config {
            backup_root: std::path::PathBuf::from("/tmp"),
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
            encryption: backup_core::config::model::EncryptionConfig::default(),
            compression: backup_core::config::model::CompressionConfig::default(),
            watched: vec![],
            safe_mode: false,
            destinations: vec![],
        };
        cfg.runtime.force_full_scan_interval_cycles = 3;
        let mut state = StoredState::default();
        state.last_run_ts = Some(1);

        state.cycles_since_full_scan = 0;
        let d0 = decide_scan(0, &state, &cfg, false);
        assert!(matches!(d0, ScanDecision::SkipClean { .. }));

        state.cycles_since_full_scan = 1;
        let d1 = decide_scan(0, &state, &cfg, false);
        assert!(matches!(d1, ScanDecision::SkipClean { .. }));

        state.cycles_since_full_scan = 2;
        let d2 = decide_scan(0, &state, &cfg, false);
        assert!(matches!(d2, ScanDecision::RunFullScan { .. }));
    }

    #[test]
    /// Purpose: Ensures destination recovery forces a full scan even when nothing is dirty.
    ///
    /// Inputs: Clean cycle state with `force_full_scan=true`.
    /// Outputs: A `RunFullScan` decision.
    /// Ties to: destination health auto-resume behavior.
    /// Side effects: None.
    /// Why: watchers do not track destination availability; a recovery must trigger a scan so backups resume promptly.
    fn decide_scan_forced_on_destination_recovery() {
        let cfg = Config {
            backup_root: std::path::PathBuf::from("/tmp"),
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
            encryption: backup_core::config::model::EncryptionConfig::default(),
            compression: backup_core::config::model::CompressionConfig::default(),
            watched: vec![],
            safe_mode: false,
            destinations: vec![],
        };
        let state = StoredState::default();
        let decision = decide_scan(0, &state, &cfg, true);
        assert!(
            matches!(decision, ScanDecision::RunFullScan { .. }),
            "expected full scan decision when destination recovers"
        );
    }

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
            encryption: backup_core::config::model::EncryptionConfig::default(),
            compression: backup_core::config::model::CompressionConfig::default(),
            watched: vec![],
            safe_mode,
            destinations: vec![backup_core::config::model::Destination {
                id: "default".into(),
                path: root.clone(),
                label: None,
                max_backups_per_file: None,
                replicate_to: vec![],
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
        assert_eq!(state.cycles_since_full_scan, 1);
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
