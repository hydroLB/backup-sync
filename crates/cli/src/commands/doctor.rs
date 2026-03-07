use anyhow::{Context, Result};
use backup_core::{
    io::{run_with_policy, BlockingIoPolicy, CancellationFlag},
    load_validated_config,
    logging::cid,
    platform::paths,
    state::store::StateStore,
    ValidationLimits,
};
use fs2::free_space;
use tracing::warn;

/// Summary: Prints a doctor report with common misconfiguration checks.
///
/// Inputs: none.
///
/// Outputs: `Ok(())` after printing diagnostic lines.
///
/// Side effects: Reads config/state, writes a probe file, and prints to stdout.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: CLI diagnostics output.
///
/// Why this exists: surface common configuration and environment issues quickly.
pub fn doctor() -> Result<()> {
    let cfg = load_validated_config().context("cli::doctor failed to load config")?;
    let io_policy = BlockingIoPolicy::from_config(&cfg);
    let limits = ValidationLimits::default();
    println!(
        "Config file: {:?}",
        paths::config_file_path().context("cli::doctor failed to resolve config path")?
    );
    println!("Backup destination: {:?}", cfg.backup_root);
    // Watched paths
    if cfg.watched.is_empty() {
        println!("Problem: no watched paths configured.");
    } else {
        let (existing, missing): (Vec<_>, Vec<_>) =
            cfg.watched.iter().partition(|w| w.path.exists());
        println!(
            "Watched paths: {} ok, {} missing",
            existing.len(),
            missing.len()
        );
        if cfg.watched.len() > limits.max_watched {
            println!(
                "Problem: too many watched entries ({}). Trim to <={} for best performance.",
                cfg.watched.len(),
                limits.max_watched
            );
        }
        for m in missing {
            println!("  Missing: {:?}", m.path);
        }
    }
    println!("interval_seconds: {}", cfg.interval_seconds);
    if cfg.interval_seconds > limits.max_interval_seconds {
        println!(
            "Problem: interval too large (>{} seconds). Pick a shorter interval.",
            limits.max_interval_seconds
        );
    }
    println!("max_backups_per_file: {}", cfg.max_backups_per_file);
    println!("max_parallel_copies: {}", cfg.max_parallel_copies);
    // Free space and writability probe
    match free_space(&cfg.backup_root) {
        Ok(bytes) => {
            println!("Free space at destination: {} bytes", bytes);
            if let Some(min_free) = cfg.min_free_space_bytes {
                if bytes < min_free {
                    println!("Problem: free space below configured minimum {}", min_free);
                } else {
                    println!("Free space meets configured minimum ({}).", min_free);
                }
            }
            // Write probe: try to create/remove a temp file
            let probe = cfg.backup_root.join(".backup_sync_probe");
            match run_with_policy(
                "cli::doctor write probe file",
                &io_policy,
                CancellationFlag::none(),
                || {
                    std::fs::write(&probe, b"probe")
                        .map_err(|error| anyhow::anyhow!(error))
                        .with_context(|| format!("cli::doctor failed writing probe {:?}", probe))
                },
            ) {
                Ok(_) => {
                    if let Err(error) = run_with_policy(
                        "cli::doctor remove probe file",
                        &io_policy,
                        CancellationFlag::none(),
                        || {
                            std::fs::remove_file(&probe)
                                .map_err(|err| anyhow::anyhow!(err))
                                .with_context(|| {
                                    format!("cli::doctor failed removing probe {:?}", probe)
                                })
                        },
                    ) {
                        let correlation_id = cid("cli-doctor");
                        warn!(
                            cid = %correlation_id,
                            action = "probe_cleanup_failed",
                            probe_path = %backup_core::logging::redact_path(&probe),
                            error = %error,
                            "cli::doctor failed to remove probe file; continuing"
                        );
                    }
                    println!("Write check: OK");
                }
                Err(e) => println!(
                    "Problem: cannot write to backup destination {:?}: {}",
                    cfg.backup_root, e
                ),
            }
        }
        Err(e) => println!("Problem reading free space at {:?}: {}", cfg.backup_root, e),
    }
    // Overlaps
    for w in &cfg.watched {
        if w.path.parent().is_none() {
            println!("Problem: watched path {:?} is a filesystem root. Pick a folder inside your home directory.", w.path);
        }
        if w.path.starts_with(&cfg.backup_root) {
            println!(
                "Problem: watched path {:?} is inside backup_root {:?}.",
                w.path, cfg.backup_root
            );
        }
        if cfg.backup_root.starts_with(&w.path) {
            println!(
                "Problem: backup_root {:?} is inside watched path {:?}.",
                cfg.backup_root, w.path
            );
        }
        if !w.path.exists() {
            println!("Problem: watched path {:?} does not exist.", w.path);
        }
    }
    // State/last error
    let state_path =
        paths::state_file_path().context("cli::doctor failed to resolve state path")?;
    let (state, _) = StateStore::load_or_default(state_path.clone())
        .context("cli::doctor failed to load state file")?;
    if let Some(err) = &state.last_error {
        println!("Last error: {}", err);
    } else {
        println!("Last error: none recorded");
    }
    println!("Last verify: {:?}", state.last_verify_ts);
    println!("State file: {:?}", state_path);
    println!("Run `backup-sync install-service --enable` to start on login if not already set.");
    Ok(())
}
