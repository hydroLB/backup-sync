use crate::commands::correlation;
use crate::commands::error::ErrorEnvelope;
use backup_core::{
    backup::versioned,
    load_validated_config,
    logging::redact_text,
    platform::paths,
    state::{store::StateStore, StoredState},
};
use serde::Serialize;
use tracing::{error, info, warn};

/// Centralize config loading and validation error handling.
fn load_and_validate_config(cid: &str) -> Result<backup_core::Config, ErrorEnvelope> {
    load_validated_config().map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_LOAD",
            format!(
                "[cid={}] load_and_validate_config failed to load config: {}",
                cid, e
            ),
        )
    })
}

/// Centralize state loading and ensure safe-mode stays in sync with config.
fn load_state_store(
    cfg: &backup_core::Config,
    cid: &str,
) -> Result<(StateStore, backup_core::state::StoredState), ErrorEnvelope> {
    let state_path = paths::state_file_path().map_err(|e| {
        ErrorEnvelope::new(
            "STATE_PATH",
            format!(
                "[cid={}] load_state_store failed to resolve state path: {}",
                cid, e
            ),
        )
    })?;
    let (mut state, store) = StateStore::load_or_default(state_path).map_err(|e| {
        ErrorEnvelope::new(
            "STATE_LOAD",
            format!("[cid={}] load_state_store failed to load state: {}", cid, e),
        )
    })?;
    state.safe_mode = cfg.safe_mode;
    Ok((store, state))
}

#[tauri::command]
/// Provide an on demand run path for the UI.
pub async fn run_now_cmd(correlation_id: Option<String>) -> Result<(), ErrorEnvelope> {
    let cid = correlation::cid("run", correlation_id);
    let task_cid = cid.clone();
    tokio::task::spawn_blocking(move || run_now_blocking(task_cid))
        .await
        .map_err(|error| blocking_task_error(&cid, "run_now_cmd", error))?
}

fn run_now_blocking(cid: String) -> Result<(), ErrorEnvelope> {
    info!(cid = %cid, action = "run_now_start", "gui backup run requested");
    let cfg = load_and_validate_config(&cid)?;
    if cfg.safe_mode {
        return Err(ErrorEnvelope::new(
            "SAFE_MODE",
            format!(
                "[cid={}] Safe mode enabled: skipping writes. Use Simulate or Verify instead.",
                cid
            ),
        ));
    }
    let (store, mut state) = load_state_store(&cfg, &cid)?;
    let result = versioned::run_backup_cycle(&cfg).map_err(|e| {
        ErrorEnvelope::new(
            "EXEC_FAILED",
            format!("[cid={}] run_now_cmd versioned backup failed: {}", cid, e),
        )
    })?;

    let mut replication_error = None;
    let has_replication_pairs = cfg.destinations.iter().any(|d| !d.replicate_to.is_empty());
    if has_replication_pairs && cfg.runtime.replication_enabled {
        match versioned::replicate_configured_stores(&cfg) {
            Ok(rep) => {
                let degraded =
                    record_replication_summary(&mut state, &rep, chrono::Utc::now().timestamp());
                info!(
                    cid = %cid,
                    action = "replication_complete",
                    message = %redact_text(&rep.message),
                    pairs_ok = rep.pairs_ok,
                    pairs_failed = rep.pairs_failed,
                    "gui replication completed"
                );
                if let Some(message) = degraded {
                    replication_error = Some(ErrorEnvelope::new(
                        "REPLICATION_DEGRADED",
                        format!(
                            "[cid={cid}] Backup completed, but replica redundancy is degraded: {}. Hint: Check failed replica destinations, restore availability, and retry.",
                            redact_text(&message)
                        ),
                    ));
                }
            }
            Err(e) => {
                let message = redact_text(&format!("{e:#}"));
                record_replication_failure(&mut state, &message, chrono::Utc::now().timestamp());
                error!(
                    cid = %cid,
                    action = "replication_failed",
                    error = %message,
                    "gui replication failed"
                );
                replication_error = Some(ErrorEnvelope::new(
                    "REPLICATION_FAILED",
                    format!(
                        "[cid={cid}] Backup completed, but replication failed: {message}. Hint: Check replica destination availability and permissions, then retry."
                    ),
                ));
            }
        }
    }

    state.last_run_ts = Some(chrono::Utc::now().timestamp());
    state.last_files_backed_up = result.versions_created;
    if replication_error.is_none() {
        state.last_error = None;
    }
    store.persist(&state).map_err(|e| {
        ErrorEnvelope::new(
            "STATE_SAVE",
            format!("[cid={}] run_now_cmd failed to persist state: {}", cid, e),
        )
    })?;
    match replication_error {
        Some(error) => {
            warn!(
                cid = %cid,
                action = "run_now_replication_degraded",
                versions_created = result.versions_created,
                code = %error.code,
                "gui backup completed with degraded redundancy"
            );
            Err(error)
        }
        None => {
            info!(
                cid = %cid,
                action = "run_now_complete",
                versions_created = result.versions_created,
                "gui backup run completed"
            );
            Ok(())
        }
    }
}

fn record_replication_summary(
    state: &mut StoredState,
    rep: &versioned::ReplicationSummary,
    timestamp: i64,
) -> Option<String> {
    state.replication_last_run_ts = Some(timestamp);
    state.replication_last_bytes_copied = rep.bytes_copied;
    state.replication_last_blobs_copied = rep.blobs_copied;
    state.replication_last_manifests_copied = rep.manifests_copied;
    state.replication_last_manifests_deleted = rep.manifests_deleted;
    state.replication_last_pairs_ok = rep.pairs_ok;
    state.replication_last_pairs_failed = rep.pairs_failed;
    state.replication_last_targets_failed = rep.targets_failed.clone();

    if rep.pairs_failed == 0 {
        state.replication_last_status = Some("ok".to_string());
        state.replication_last_error = None;
        return None;
    }

    state.replication_last_status = Some("degraded".to_string());
    state.replication_last_error = Some(rep.message.clone());
    state.last_error = Some(format!("Replication degraded: {}", rep.message));
    Some(rep.message.clone())
}

fn record_replication_failure(state: &mut StoredState, message: &str, timestamp: i64) {
    state.replication_last_run_ts = Some(timestamp);
    state.replication_last_status = Some("failed".to_string());
    state.replication_last_error = Some(message.to_string());
    state.replication_last_bytes_copied = 0;
    state.replication_last_blobs_copied = 0;
    state.replication_last_manifests_copied = 0;
    state.replication_last_manifests_deleted = 0;
    state.replication_last_pairs_ok = 0;
    state.replication_last_pairs_failed = 0;
    state.replication_last_targets_failed.clear();
    state.last_error = Some(format!("Replication failed: {message}"));
}

#[derive(Serialize)]
pub struct SimulationResult {
    pub items: usize,
    pub bytes: u64,
    pub sample: Vec<String>,
    pub message: String,
}

#[tauri::command]
/// Allow users to inspect changes before running a backup.
pub async fn run_simulate_cmd(
    correlation_id: Option<String>,
) -> Result<SimulationResult, ErrorEnvelope> {
    let cid = correlation::cid("sim", correlation_id);
    let task_cid = cid.clone();
    tokio::task::spawn_blocking(move || run_simulate_blocking(task_cid))
        .await
        .map_err(|error| blocking_task_error(&cid, "run_simulate_cmd", error))?
}

fn run_simulate_blocking(cid: String) -> Result<SimulationResult, ErrorEnvelope> {
    info!(cid = %cid, action = "simulate_start", "gui simulation requested");
    let cfg = load_and_validate_config(&cid)?;
    let sim = versioned::simulate_backup_cycle(&cfg).map_err(|e| {
        ErrorEnvelope::new(
            "SIMULATE_FAILED",
            format!("[cid={}] run_simulate_cmd failed: {}", cid, e),
        )
    })?;
    let mut message = format!(
        "would_create_versions={}/{} changes=+{} ~{} -{} blobs_to_write={} bytes_to_write={}",
        sim.versions_would_create,
        sim.watched,
        sim.adds,
        sim.modifies,
        sim.deletes,
        sim.blobs_to_write,
        sim.bytes_to_write
    );
    if sim.read_failures > 0 || sim.snapshot_errors > 0 {
        message.push_str(&format!(
            " read_failures={} snapshot_errors={}",
            sim.read_failures, sim.snapshot_errors
        ));
        warn!(
            cid = %cid,
            action = "simulate_quality_warnings",
            read_failures = sim.read_failures,
            snapshot_errors = sim.snapshot_errors,
            "gui simulation completed with scan warnings"
        );
    }
    info!(
        cid = %cid,
        action = "simulate_complete",
        items = sim.items,
        bytes = sim.bytes_to_write,
        "gui simulation completed"
    );
    Ok(SimulationResult {
        items: sim.items,
        bytes: sim.bytes_to_write,
        sample: sim.sample,
        message,
    })
}

fn blocking_task_error(cid: &str, command: &str, error: tokio::task::JoinError) -> ErrorEnvelope {
    ErrorEnvelope::new(
        "BLOCKING_TASK_FAILED",
        format!("[cid={cid}] {command} blocking task failed: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::{record_replication_failure, record_replication_summary};
    use backup_core::{backup::versioned::ReplicationSummary, state::StoredState};

    #[test]
    fn degraded_summary_keeps_replication_and_run_errors_visible() {
        let mut state = StoredState::default();
        let summary = ReplicationSummary {
            pairs_attempted: 2,
            pairs_ok: 1,
            pairs_failed: 1,
            targets_failed: vec!["secondary".to_string()],
            message: "replication pairs_ok=1/2 targets_failed=1".to_string(),
            ..ReplicationSummary::default()
        };

        assert!(record_replication_summary(&mut state, &summary, 42).is_some());
        assert_eq!(state.replication_last_status.as_deref(), Some("degraded"));
        assert_eq!(state.replication_last_pairs_failed, 1);
        assert_eq!(state.replication_last_targets_failed, vec!["secondary"]);
        assert!(state.replication_last_error.is_some());
        assert!(state.last_error.is_some());
    }

    #[test]
    fn failed_replication_replaces_stale_success_metrics() {
        let mut state = StoredState {
            replication_last_bytes_copied: 99,
            replication_last_pairs_ok: 2,
            replication_last_targets_failed: vec!["stale".to_string()],
            ..StoredState::default()
        };

        record_replication_failure(&mut state, "replica unavailable", 84);

        assert_eq!(state.replication_last_status.as_deref(), Some("failed"));
        assert_eq!(state.replication_last_bytes_copied, 0);
        assert_eq!(state.replication_last_pairs_ok, 0);
        assert!(state.replication_last_targets_failed.is_empty());
        assert!(state.last_error.as_deref().unwrap().contains("unavailable"));
    }
}
