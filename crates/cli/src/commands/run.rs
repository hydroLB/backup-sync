use anyhow::{Context, Result};
use backup_core::{
    backup::versioned,
    load_validated_config,
    logging::redact_text,
    platform::paths,
    state::{store::StateStore, StoredState},
};

const REPLICATION_ACTION: &str =
    "One or more configured replica destinations are unreachable; inspect replication status, restore destination availability, and retry.";

/// Allow manual execution of backup cycles from the CLI.
pub async fn run_once(dry_run: bool) -> Result<()> {
    let cfg = load_validated_config().context("cli::run_once failed to load config")?;

    if !dry_run && cfg.safe_mode {
        anyhow::bail!(
            "cli::run_once safe mode enabled: simulation/verify only. Disable safe mode to run backups."
        );
    }

    let state_path =
        paths::state_file_path().context("cli::run_once failed to resolve state path")?;
    let (mut state, store) = StateStore::load_or_default(state_path)
        .context("cli::run_once failed to load state file")?;
    if !dry_run && state.safe_mode {
        anyhow::bail!(
            "cli::run_once safe mode enabled in state: simulation/verify only. Disable safe mode to run backups."
        );
    }

    if dry_run {
        let sim = versioned::simulate_backup_cycle(&cfg)
            .context("cli::run_once failed to simulate versioned backup changes")?;
        println!(
            "Simulation: watched={} would_create_versions={} changes=+{} ~{} -{} items={} blobs_to_write={} bytes_to_write={}",
            sim.watched,
            sim.versions_would_create,
            sim.adds,
            sim.modifies,
            sim.deletes,
            sim.items,
            sim.blobs_to_write,
            sim.bytes_to_write
        );
        if sim.read_failures > 0 || sim.snapshot_errors > 0 {
            println!(
                "Warnings: read_failures={} snapshot_errors={}",
                sim.read_failures, sim.snapshot_errors
            );
        }
        if !sim.sample.is_empty() {
            println!("Sample:");
            for s in sim.sample.iter() {
                println!("  {}", s);
            }
        }
        return Ok(());
    }

    let res = versioned::run_backup_cycle(&cfg).context("cli::run_once versioned backup failed")?;
    if res.versions_created == 0 {
        println!("No changes detected; nothing to back up.");
    } else {
        println!(
            "Backup complete: folders_scanned={} versions_created={} blobs_written={} bytes_written={}",
            res.folders_scanned, res.versions_created, res.blobs_written, res.bytes_written
        );
    }

    let mut replication_error = None;
    let has_replication_pairs = cfg.destinations.iter().any(|d| !d.replicate_to.is_empty());
    if has_replication_pairs && cfg.runtime.replication_enabled {
        match versioned::replicate_configured_stores(&cfg) {
            Ok(rep) => {
                let degraded =
                    record_replication_summary(&mut state, &rep, chrono::Utc::now().timestamp());
                if rep.pairs_attempted > 0 {
                    println!("Replication: {}", rep.message);
                }
                if let Some(message) = degraded {
                    replication_error = Some(anyhow::anyhow!(
                        "cli::run_once replication degraded: {message}. {REPLICATION_ACTION}"
                    ));
                }
            }
            Err(e) => {
                let message = redact_text(&format!("{e:#}"));
                println!("Replication: failed ({message})");
                record_replication_failure(&mut state, &message, chrono::Utc::now().timestamp());
                replication_error = Some(e.context(format!(
                    "cli::run_once replication failed. {REPLICATION_ACTION}"
                )));
            }
        }
    }

    state.last_run_ts = Some(chrono::Utc::now().timestamp());
    state.last_files_backed_up = res.versions_created;
    if replication_error.is_none() {
        state.last_error = None;
    }
    store
        .persist(&state)
        .context("cli::run_once failed to persist state after backup")?;
    match replication_error {
        Some(error) => Err(error),
        None => Ok(()),
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

#[cfg(test)]
mod tests {
    use super::{record_replication_failure, record_replication_summary};
    use backup_core::{backup::versioned::ReplicationSummary, state::StoredState};

    #[test]
    fn degraded_replication_is_preserved_as_a_run_error() {
        let mut state = StoredState::default();
        let summary = ReplicationSummary {
            pairs_attempted: 2,
            pairs_ok: 1,
            pairs_failed: 1,
            targets_failed: vec!["mirror-b".to_string()],
            message: "replication pairs_ok=1/2 targets_failed=1".to_string(),
            ..ReplicationSummary::default()
        };

        let degraded = record_replication_summary(&mut state, &summary, 42);

        assert!(degraded.is_some());
        assert_eq!(state.replication_last_status.as_deref(), Some("degraded"));
        assert_eq!(state.replication_last_pairs_failed, 1);
        assert_eq!(state.replication_last_targets_failed, vec!["mirror-b"]);
        assert!(state.last_error.as_deref().unwrap().contains("degraded"));
    }

    #[test]
    fn failed_replication_clears_stale_counters_and_records_error() {
        let mut state = StoredState {
            replication_last_pairs_ok: 3,
            replication_last_pairs_failed: 1,
            replication_last_targets_failed: vec!["stale".to_string()],
            ..StoredState::default()
        };

        record_replication_failure(&mut state, "destination unavailable", 84);

        assert_eq!(state.replication_last_status.as_deref(), Some("failed"));
        assert_eq!(state.replication_last_pairs_ok, 0);
        assert!(state.replication_last_targets_failed.is_empty());
        assert!(state.last_error.as_deref().unwrap().contains("unavailable"));
    }
}
