use super::logging;
use anyhow::{Context, Result};
use backup_core::{
    backup::versioned,
    fs::watching::{debounce::debounce_and_take, debounce_duration, DirtySet},
    io::{run_with_policy, BlockingIoPolicy, CancellationFlag},
    Config, HashingTuning, StateStore, StoredState,
};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinError;

// Serialize merge-and-persist commits without holding the shared state mutex during filesystem IO.
// This prevents an older snapshot from finishing after and overwriting a newer commit.
static STATE_COMMIT_LOCK: Mutex<()> = Mutex::const_new(());

#[derive(Debug, Clone)]
struct DestinationWriteHealth {
    id: String,
    ok: bool,
    message: String,
}

#[derive(Debug, Clone)]
struct DestinationPauseTransition {
    pause_reason: Option<String>,
    destination_recovered: bool,
}

fn apply_backup_cycle_result(
    state: &mut StoredState,
    result: &versioned::BackupCycleResult,
    now: i64,
) {
    state.last_files_backed_up = result.versions_created;
    state.last_run_ts = Some(now);
    state.last_error = None;
    state.cycles_since_full_scan = 0;
    if let Some(warning) = result.safety_warnings.last().cloned() {
        state.last_safety_warning = Some(warning);
    }
}

fn safety_warning_eq(
    left: &Option<backup_core::SafetyWarning>,
    right: &Option<backup_core::SafetyWarning>,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            left.ts == right.ts
                && left.message == right.message
                && left.watched_path == right.watched_path
                && left.kept_version_id == right.kept_version_id
        }
        _ => false,
    }
}

/// Merge cycle-owned fields without reverting updates made by IPC or verification while the
/// filesystem work was running.
fn merge_cycle_state(current: &mut StoredState, baseline: &StoredState, mut cycle: StoredState) {
    cycle.safe_mode = current.safe_mode;
    cycle.last_verify_ts = current.last_verify_ts;
    cycle.last_verify_status = current.last_verify_status.clone();
    cycle.last_verify_issues = current.last_verify_issues;
    cycle.last_scrub_full_ts = current.last_scrub_full_ts;
    if !safety_warning_eq(&current.last_safety_warning, &baseline.last_safety_warning) {
        cycle.last_safety_warning = current.last_safety_warning.clone();
    }
    *current = cycle;
}

fn classify_join_error(operation: &str, error: JoinError) -> anyhow::Error {
    let kind = if error.is_panic() {
        "panicked"
    } else if error.is_cancelled() {
        "was cancelled"
    } else {
        "failed"
    };
    anyhow::anyhow!("daemon blocking task {operation} {kind}: {error}")
}

async fn persist_snapshot(
    store: &StateStore,
    state: StoredState,
    context: &'static str,
) -> Result<()> {
    let store = store.clone();
    tokio::task::spawn_blocking(move || store.persist(&state))
        .await
        .map_err(|error| classify_join_error("state persistence", error))?
        .context(context)
}

async fn commit_cycle_state(
    store: &StateStore,
    shared_state: &Arc<Mutex<StoredState>>,
    baseline: &StoredState,
    cycle: StoredState,
    context: &'static str,
) -> Result<StoredState> {
    let _commit_guard = STATE_COMMIT_LOCK.lock().await;
    let snapshot = {
        let mut current = shared_state.lock().await;
        merge_cycle_state(&mut current, baseline, cycle);
        current.clone()
    };
    persist_snapshot(store, snapshot.clone(), context).await?;
    Ok(snapshot)
}

fn apply_replication_success(
    state: &mut StoredState,
    summary: &versioned::ReplicationSummary,
    now: i64,
) {
    state.replication_last_run_ts = Some(now);
    state.replication_last_bytes_copied = summary.bytes_copied;
    state.replication_last_blobs_copied = summary.blobs_copied;
    state.replication_last_manifests_copied = summary.manifests_copied;
    state.replication_last_manifests_deleted = summary.manifests_deleted;
    state.replication_last_pairs_ok = summary.pairs_ok;
    state.replication_last_pairs_failed = summary.pairs_failed;
    state.replication_last_targets_failed = summary.targets_failed.clone();
    state.replication_last_status = Some(if summary.pairs_failed == 0 {
        "ok".to_string()
    } else {
        "degraded".to_string()
    });
    state.replication_last_error = if summary.pairs_failed == 0 {
        None
    } else {
        Some(summary.message.clone())
    };
}

fn apply_replication_failure(state: &mut StoredState, error: &anyhow::Error, now: i64) {
    state.replication_last_run_ts = Some(now);
    state.replication_last_status = Some("failed".to_string());
    state.replication_last_error = Some(format!("{error:#}"));
}

fn apply_destination_health(
    state: &mut StoredState,
    dest_health: &[DestinationWriteHealth],
    now: i64,
) -> DestinationPauseTransition {
    let unavailable: Vec<String> = dest_health
        .iter()
        .filter(|destination| !destination.ok)
        .map(|destination| destination.id.clone())
        .collect();
    state.destination_unavailable_ids = unavailable.clone();

    let pause_reason = if unavailable.is_empty() {
        None
    } else {
        let details: Vec<String> = dest_health
            .iter()
            .filter(|destination| !destination.ok)
            .map(|destination| format!("{} ({})", destination.id, destination.message))
            .collect();
        Some(format!(
            "Destination unavailable: {}. Writes paused and will auto-resume when the destination returns.",
            details.join(", ")
        ))
    };

    let was_paused = state.destination_paused;
    state.destination_paused = pause_reason.is_some();
    state.destination_pause_reason = pause_reason.clone();
    if state.destination_paused && (!was_paused || state.destination_last_unavailable_ts.is_none())
    {
        state.destination_last_unavailable_ts = Some(now);
    }

    DestinationPauseTransition {
        pause_reason,
        destination_recovered: was_paused && !state.destination_paused,
    }
}

/// Run one daemon backup cycle, including destination gating, scan decisions, backup, and replication.
pub async fn run_cycle(
    cfg: &Config,
    dirty: &DirtySet,
    store: &StateStore,
    shared_state: &Arc<Mutex<StoredState>>,
) -> Result<()> {
    let cycle_cid = logging::cid("cycle");
    logging::log_info("run_cycle_start", &cycle_cid, None, "starting backup cycle");
    let baseline = shared_state.lock().await.clone();
    let mut state = baseline.clone();
    state.last_dirty_count = dirty.0.lock().len();

    let now = chrono::Utc::now().timestamp();
    let probe_cfg = cfg.clone();
    let dest_health = tokio::task::spawn_blocking(move || {
        let needed_destination_ids = required_destination_ids(&probe_cfg);
        probe_required_destinations_for_write(&probe_cfg, &needed_destination_ids)
    })
    .await
    .map_err(|error| classify_join_error("destination probe", error))?;

    // Stage 1: update pause/recovery state from destination health before deciding whether the
    // engine is even allowed to run this cycle.
    let transition = apply_destination_health(&mut state, &dest_health, now);
    if transition.destination_recovered {
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

    if let Some(reason) = transition.pause_reason.as_deref() {
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
        commit_cycle_state(
            store,
            shared_state,
            &baseline,
            state,
            "daemon::runtime::run_cycle failed to persist state after destination pause",
        )
        .await?;
        return Ok(());
    }

    if apply_safe_mode(cfg, &mut state, store, &cycle_cid, shared_state)
        .await
        .with_context(|| "daemon::runtime::run_cycle failed during safe mode check")?
    {
        return Ok(());
    }

    state.last_dirty_count = drain_dirty_count(cfg, dirty).await;

    // Stage 2: use watcher dirtiness plus periodic forced scans to decide whether to pay the cost
    // of a full versioned scan on this cycle.
    match decide_scan(
        state.last_dirty_count,
        &state,
        cfg,
        transition.destination_recovered,
    ) {
        ScanDecision::SkipClean { force_due_in } => {
            state.cycles_since_full_scan = state.cycles_since_full_scan.saturating_add(1);
            state.last_run_ts = Some(now);
            state.last_files_backed_up = 0;
            state.last_error = None;
            commit_cycle_state(
                store,
                shared_state,
                &baseline,
                state,
                "daemon::runtime::run_cycle failed to persist state after clean skip",
            )
            .await?;
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

    // Stage 3: run the core engine, then fold backup and replication outcomes back into daemon
    // state for UI/status consumers.
    let engine_cfg = cfg.clone();
    let (result, replication) = tokio::task::spawn_blocking(move || {
        let result = versioned::run_backup_cycle(&engine_cfg)
            .with_context(|| "daemon::runtime::run_cycle versioned backup cycle failed")?;
        let has_replication_pairs = engine_cfg
            .destinations
            .iter()
            .any(|d| !d.replicate_to.is_empty());
        let replication = if has_replication_pairs && engine_cfg.runtime.replication_enabled {
            Some(versioned::replicate_configured_stores(&engine_cfg))
        } else {
            None
        };
        Ok::<_, anyhow::Error>((result, replication))
    })
    .await
    .map_err(|error| classify_join_error("backup and replication", error))??;
    apply_backup_cycle_result(&mut state, &result, now);

    if let Some(replication) = replication {
        match replication {
            Ok(summary) => {
                apply_replication_success(&mut state, &summary, now);
                logging::log_info("replication_complete", &cycle_cid, None, &summary.message);
            }
            Err(error) => {
                apply_replication_failure(&mut state, &error, now);
                logging::log_warn(
                    "replication_failed",
                    &cycle_cid,
                    None,
                    &format!("replication error: {error:#}"),
                );
            }
        }
    }

    commit_cycle_state(
        store,
        shared_state,
        &baseline,
        state,
        "daemon::runtime::run_cycle failed to persist state after backup",
    )
    .await?;

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

/// Decide whether this cycle needs a full scan or can safely skip on watcher cleanliness.
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

/// Drain the watcher dirty set and return the debounced count used for scan decisions and status.
async fn drain_dirty_count(cfg: &Config, dirty: &DirtySet) -> usize {
    if dirty.0.lock().is_empty() {
        return 0;
    }
    let dirty_paths = debounce_and_take(dirty, debounce_duration(&cfg.runtime)).await;
    dirty_paths.len()
}

/// Return the destination ids referenced by enabled watched paths.
fn required_destination_ids(cfg: &Config) -> HashSet<&str> {
    cfg.watched
        .iter()
        .filter(|w| w.enabled)
        .map(|w| w.destination_id.as_str())
        .collect()
}

/// Probe the destinations in active use and report whether each one is write-ready.
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
        .map(|d| probe_destination_for_write(cfg, d))
        .collect()
}

/// Probe one destination by checking the path shape and attempting to create the store root.
fn probe_destination_for_write(
    cfg: &Config,
    d: &backup_core::config::model::Destination,
) -> DestinationWriteHealth {
    let io_policy = BlockingIoPolicy::from_config(cfg);
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
    if let Err(e) = run_with_policy(
        "daemon::runtime::probe_destination_for_write create probe directory",
        &io_policy,
        CancellationFlag::none(),
        || {
            fs::create_dir_all(&probe_dir).map_err(|error| {
                anyhow::anyhow!(error)
                    .context("daemon::runtime::probe_destination_for_write failed create_dir_all")
            })
        },
    ) {
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

/// Short-circuit a cycle when safe mode is active while still updating observable state.
pub(crate) async fn apply_safe_mode(
    cfg: &Config,
    state: &mut StoredState,
    store: &StateStore,
    cid: &str,
    shared_state: &Arc<Mutex<StoredState>>,
) -> Result<bool> {
    let baseline = state.clone();
    state.safe_mode = shared_state.lock().await.safe_mode;
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
        *state = commit_cycle_state(
            store,
            shared_state,
            &baseline,
            state.clone(),
            "daemon::runtime::apply_safe_mode failed to persist state in safe mode",
        )
        .await?;
        return Ok(true);
    }
    Ok(false)
}

enum VerifyOutcome {
    Skipped(Vec<String>),
    Scrubbed(Result<versioned::ScrubResult>),
}

struct VerifyStateUpdate {
    last_verify_ts: Option<i64>,
    last_verify_status: Option<String>,
    last_verify_issues: Option<usize>,
    last_scrub_full_ts: Option<i64>,
}

fn apply_verify_update(state: &mut StoredState, update: VerifyStateUpdate) {
    state.last_verify_ts = update.last_verify_ts;
    state.last_verify_status = update.last_verify_status;
    state.last_verify_issues = update.last_verify_issues;
    if let Some(timestamp) = update.last_scrub_full_ts {
        state.last_scrub_full_ts = Some(timestamp);
    }
}

/// Keep verification results up to date for UI and logs without holding state across a scrub.
pub async fn run_verify_cycle(
    cfg: &Config,
    store: &StateStore,
    shared_state: &Arc<Mutex<StoredState>>,
    hashing: &HashingTuning,
) -> Result<()> {
    let cid = logging::cid("verify");
    let now = chrono::Utc::now().timestamp();
    let last_scrub_full_ts = shared_state.lock().await.last_scrub_full_ts;
    let full_due = match last_scrub_full_ts {
        None => true,
        Some(ts) => now.saturating_sub(ts) >= cfg.runtime.scrub_full_interval_seconds as i64,
    };
    let mode = if full_due {
        versioned::ScrubMode::Full
    } else {
        versioned::ScrubMode::Sampled
    };
    let verify_cfg = cfg.clone();
    let verify_hashing = hashing.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        let needed_destination_ids = required_destination_ids(&verify_cfg);
        let unavailable: Vec<_> = verify_cfg
            .destinations
            .iter()
            .filter(|d| needed_destination_ids.contains(d.id.as_str()))
            .filter(|d| !d.path.exists() || !d.path.is_dir())
            .map(|d| d.id.clone())
            .collect();
        if !unavailable.is_empty() {
            return VerifyOutcome::Skipped(unavailable);
        }
        VerifyOutcome::Scrubbed(versioned::scrub_versioned_store(
            &verify_cfg,
            &verify_hashing,
            mode,
            verify_cfg.runtime.scrub_sample_blobs,
            verify_cfg.runtime.scrub_sample_versions_per_source,
            now as u64,
        ))
    })
    .await
    .map_err(|error| classify_join_error("versioned store scrub", error))?;

    let mut update = VerifyStateUpdate {
        last_verify_ts: Some(now),
        last_verify_status: None,
        last_verify_issues: None,
        last_scrub_full_ts: None,
    };
    match outcome {
        VerifyOutcome::Skipped(unavailable) => {
            let msg = format!(
                "skipped: destination unavailable ({})",
                unavailable.join(", ")
            );
            update.last_verify_status = Some(msg.clone());
            logging::log_warn("verify_skipped", &cid, None, &msg);
        }
        VerifyOutcome::Scrubbed(Ok(res)) => {
            let bad = res.hash_mismatches + res.missing_blobs + res.manifests_bad;
            let ok = res.blobs_hashed.saturating_sub(res.hash_mismatches);
            let mode_label = match res.mode {
                versioned::ScrubMode::Sampled => "sampled",
                versioned::ScrubMode::Full => "full",
            };
            update.last_verify_issues = Some(bad);
            update.last_verify_status = Some(if bad == 0 {
                format!("ok ({mode_label})")
            } else {
                format!("issues_detected ({mode_label})")
            });
            if res.mode == versioned::ScrubMode::Full {
                update.last_scrub_full_ts = Some(now);
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
        VerifyOutcome::Scrubbed(Err(error)) => {
            update.last_verify_status = Some(format!("failed: {error}"));
            logging::log_warn(
                "verify_failed",
                &cid,
                None,
                &format!("verify error: {error:?}"),
            );
        }
    }
    let _commit_guard = STATE_COMMIT_LOCK.lock().await;
    let snapshot = {
        let mut state = shared_state.lock().await;
        apply_verify_update(&mut state, update);
        state.clone()
    };
    persist_snapshot(
        store,
        snapshot,
        "daemon::runtime::run_verify_cycle failed to persist state after verify",
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    /// The first cycle must establish baseline state even if watchers have not fired yet.
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
        let state = StoredState {
            start_ts: Some(10),
            last_run_ts: Some(1),
            ..StoredState::default()
        };
        let decision = decide_scan(0, &state, &cfg, false);
        assert!(
            matches!(decision, ScanDecision::RunFullScan { .. }),
            "expected full scan decision on first cycle"
        );
    }

    #[test]
    /// Avoid expensive scans when nothing changed while still providing a periodic safety scan.
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
        let mut state = StoredState {
            last_run_ts: Some(1),
            ..StoredState::default()
        };

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
    /// Watchers do not track destination availability; a recovery must trigger a scan so backups resume promptly.
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

    #[test]
    fn apply_destination_health_marks_pause_and_reason() {
        let mut state = StoredState::default();
        let health = vec![DestinationWriteHealth {
            id: "usb".to_string(),
            ok: false,
            message: "destination folder not found".to_string(),
        }];

        let transition = apply_destination_health(&mut state, &health, 42);

        assert!(state.destination_paused);
        assert_eq!(state.destination_last_unavailable_ts, Some(42));
        assert_eq!(state.destination_unavailable_ids, vec!["usb".to_string()]);
        assert!(!transition.destination_recovered);
        assert!(transition
            .pause_reason
            .as_deref()
            .unwrap_or_default()
            .contains("usb (destination folder not found)"));
    }

    #[test]
    fn apply_destination_health_detects_recovery() {
        let mut state = StoredState {
            destination_paused: true,
            destination_pause_reason: Some("Destination unavailable: usb".to_string()),
            destination_unavailable_ids: vec!["usb".to_string()],
            ..StoredState::default()
        };

        let transition = apply_destination_health(&mut state, &[], 100);

        assert!(!state.destination_paused);
        assert!(state.destination_unavailable_ids.is_empty());
        assert!(transition.destination_recovered);
        assert!(transition.pause_reason.is_none());
    }

    #[test]
    fn apply_backup_cycle_result_updates_state_summary() {
        let mut state = StoredState::default();
        let result = versioned::BackupCycleResult {
            folders_scanned: 2,
            versions_created: 3,
            blobs_written: 4,
            bytes_written: 5,
            safety_warnings: vec![backup_core::state::models::SafetyWarning {
                ts: 70,
                message: "check shrink".to_string(),
                watched_path: Some("/src".to_string()),
                kept_version_id: Some("v1".to_string()),
            }],
        };

        apply_backup_cycle_result(&mut state, &result, 77);

        assert_eq!(state.last_run_ts, Some(77));
        assert_eq!(state.last_files_backed_up, 3);
        assert_eq!(state.cycles_since_full_scan, 0);
        assert_eq!(state.last_error, None);
        assert_eq!(
            state
                .last_safety_warning
                .as_ref()
                .map(|warning| warning.message.as_str()),
            Some("check shrink")
        );
    }

    fn warning(message: &str, ts: i64) -> backup_core::SafetyWarning {
        backup_core::SafetyWarning {
            ts,
            message: message.to_string(),
            watched_path: Some("/src".to_string()),
            kept_version_id: Some("v1".to_string()),
        }
    }

    #[test]
    fn cycle_merge_preserves_concurrent_ipc_and_verify_updates() {
        let baseline = StoredState {
            safe_mode: false,
            last_verify_ts: Some(10),
            last_verify_status: Some("old".into()),
            last_verify_issues: Some(1),
            last_scrub_full_ts: Some(9),
            last_safety_warning: Some(warning("old warning", 10)),
            ..StoredState::default()
        };
        let mut cycle = baseline.clone();
        cycle.last_run_ts = Some(100);
        cycle.last_safety_warning = Some(warning("cycle warning", 100));
        let mut current = baseline.clone();
        current.safe_mode = true;
        current.last_verify_ts = Some(90);
        current.last_verify_status = Some("ok (full)".into());
        current.last_verify_issues = Some(0);
        current.last_scrub_full_ts = Some(90);
        current.last_safety_warning = None;

        merge_cycle_state(&mut current, &baseline, cycle);

        assert_eq!(current.last_run_ts, Some(100));
        assert!(current.safe_mode);
        assert_eq!(current.last_verify_ts, Some(90));
        assert_eq!(current.last_verify_status.as_deref(), Some("ok (full)"));
        assert_eq!(current.last_verify_issues, Some(0));
        assert_eq!(current.last_scrub_full_ts, Some(90));
        assert!(current.last_safety_warning.is_none());
    }

    #[test]
    fn cycle_merge_accepts_cycle_warning_only_when_warning_was_unchanged() {
        let baseline = StoredState {
            last_safety_warning: Some(warning("old", 1)),
            ..StoredState::default()
        };
        let mut cycle = baseline.clone();
        cycle.last_safety_warning = Some(warning("from cycle", 2));

        let mut unchanged = baseline.clone();
        merge_cycle_state(&mut unchanged, &baseline, cycle.clone());
        assert_eq!(
            unchanged
                .last_safety_warning
                .as_ref()
                .map(|warning| warning.message.as_str()),
            Some("from cycle")
        );

        let mut concurrently_replaced = baseline.clone();
        concurrently_replaced.last_safety_warning = Some(warning("from ipc", 3));
        merge_cycle_state(&mut concurrently_replaced, &baseline, cycle);
        assert_eq!(
            concurrently_replaced
                .last_safety_warning
                .as_ref()
                .map(|warning| warning.message.as_str()),
            Some("from ipc")
        );
    }

    #[test]
    fn verify_update_changes_only_verify_owned_fields() {
        let mut state = StoredState {
            safe_mode: true,
            last_run_ts: Some(7),
            last_safety_warning: Some(warning("keep", 1)),
            last_scrub_full_ts: Some(4),
            ..StoredState::default()
        };

        apply_verify_update(
            &mut state,
            VerifyStateUpdate {
                last_verify_ts: Some(8),
                last_verify_status: Some("ok (sampled)".into()),
                last_verify_issues: Some(0),
                last_scrub_full_ts: None,
            },
        );

        assert!(state.safe_mode);
        assert_eq!(state.last_run_ts, Some(7));
        assert_eq!(state.last_scrub_full_ts, Some(4));
        assert_eq!(state.last_verify_ts, Some(8));
        assert_eq!(
            state
                .last_safety_warning
                .as_ref()
                .map(|warning| warning.message.as_str()),
            Some("keep")
        );
    }

    #[test]
    fn apply_replication_success_marks_degraded_when_pairs_fail() {
        let mut state = StoredState::default();
        let summary = versioned::ReplicationSummary {
            pairs_attempted: 2,
            pairs_ok: 1,
            pairs_failed: 1,
            manifests_copied: 3,
            blobs_copied: 4,
            bytes_copied: 5,
            manifests_deleted: 6,
            targets_failed: vec!["replica-b".to_string()],
            message: "replication degraded".to_string(),
        };

        apply_replication_success(&mut state, &summary, 88);

        assert_eq!(state.replication_last_run_ts, Some(88));
        assert_eq!(state.replication_last_status.as_deref(), Some("degraded"));
        assert_eq!(state.replication_last_pairs_ok, 1);
        assert_eq!(state.replication_last_pairs_failed, 1);
        assert_eq!(
            state.replication_last_targets_failed,
            vec!["replica-b".to_string()]
        );
        assert_eq!(
            state.replication_last_error.as_deref(),
            Some("replication degraded")
        );
    }

    /// Keep test setup for safe mode behavior consistent.
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
    /// Avoid writes while reporting safe mode status.
    async fn safe_mode_short_circuits_and_sets_state() {
        let (state, skipped) = apply_safe_mode_case(true).await;
        assert!(skipped, "safe mode should short-circuit execution");
        assert_eq!(state.last_files_backed_up, 0);
        assert!(state.last_error.unwrap_or_default().contains("Safe mode"));
        assert_eq!(state.cycles_since_full_scan, 1);
    }

    #[tokio::test]
    /// Allow backups when safe mode is off.
    async fn safe_mode_disabled_continues() {
        let (_state, skipped) = apply_safe_mode_case(false).await;
        assert!(!skipped, "should continue when safe mode disabled");
    }
}
