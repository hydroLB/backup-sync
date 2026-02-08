use anyhow::{Context, Result};
use backup_core::{
    backup::versioned, load_config, platform::paths, state::store::StateStore, validate,
};
use fs2::free_space;
use std::path::Path;

/// Purpose: Executes a one off backup run with optional dry run mode.
///
/// Inputs: a dry run flag.
/// Outputs: `Ok(())` when the operation completes.
/// Ties to: CLI command handling and backup execution.
/// Side effects: Reads config/state, scans files, and writes backups when not dry run.
/// Why: allow manual execution of backup cycles from the CLI.
pub async fn run_once(dry_run: bool) -> Result<()> {
    let cfg = load_config().context("cli::run_once failed to load config")?;
    validate(&cfg).context("cli::run_once config validation failed")?;

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

    // Pre-flight free space check (write path only).
    if let Some(min_free) = cfg.min_free_space_bytes {
        let check_path: &Path = &cfg.backup_root;
        match free_space(check_path) {
            Ok(free) if free < min_free => {
                anyhow::bail!(
                    "cli::run_once free space {} below configured minimum {} at {:?}",
                    free,
                    min_free,
                    check_path
                );
            }
            Ok(_) => {}
            Err(e) => anyhow::bail!(
                "cli::run_once failed to read free space at {:?}: {}",
                check_path,
                e
            ),
        }
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

    let has_replication_pairs = cfg.destinations.iter().any(|d| !d.replicate_to.is_empty());
    if has_replication_pairs && cfg.runtime.replication_enabled {
        match versioned::replicate_configured_stores(&cfg) {
            Ok(rep) => {
                if rep.pairs_attempted > 0 {
                    println!("Replication: {}", rep.message);
                }
            }
            Err(e) => {
                println!("Replication: failed ({:#})", e);
            }
        }
    }

    state.last_run_ts = Some(chrono::Utc::now().timestamp());
    state.last_files_backed_up = res.versions_created;
    state.last_error = None;
    store
        .persist(&state)
        .context("cli::run_once failed to persist state after backup")?;
    Ok(())
}
