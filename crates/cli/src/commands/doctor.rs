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
use std::{
    fs::OpenOptions,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::warn;

const PROBE_PREFIX: &str = ".backup_sync_probe-";
const PROBE_CREATE_ATTEMPTS: u64 = 16;
static PROBE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct WritabilityProbe {
    path: Option<PathBuf>,
}

impl WritabilityProbe {
    fn create(destination: &Path) -> Result<Self> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let sequence_start = PROBE_SEQUENCE.fetch_add(PROBE_CREATE_ATTEMPTS, Ordering::Relaxed);

        for offset in 0..PROBE_CREATE_ATTEMPTS {
            let filename = format!(
                "{PROBE_PREFIX}{}-{timestamp}-{}",
                std::process::id(),
                sequence_start.wrapping_add(offset)
            );
            let path = destination.join(filename);
            let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => file,
                Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(error).context(
                        "cli::doctor failed to create an exclusive writability probe in the backup destination",
                    )
                }
            };

            if let Err(write_error) = file.write_all(b"probe") {
                drop(file);
                let cleanup_error = std::fs::remove_file(&path).err();
                return match cleanup_error {
                    Some(cleanup_error) => Err(anyhow::anyhow!(write_error)).context(format!(
                        "cli::doctor failed to write the writability probe and cleanup also failed: {cleanup_error}"
                    )),
                    None => Err(anyhow::anyhow!(write_error))
                        .context("cli::doctor failed to write the writability probe"),
                };
            }

            return Ok(Self { path: Some(path) });
        }

        anyhow::bail!(
            "cli::doctor could not reserve a unique writability probe after {PROBE_CREATE_ATTEMPTS} attempts"
        )
    }

    fn remove(&mut self) -> Result<()> {
        let Some(path) = self.path.as_ref() else {
            return Ok(());
        };
        match std::fs::remove_file(path) {
            Ok(()) => {
                self.path = None;
                Ok(())
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                self.path = None;
                Ok(())
            }
            Err(error) => Err(error).context("cli::doctor failed to remove its writability probe"),
        }
    }

    fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

impl Drop for WritabilityProbe {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Surface common configuration and environment issues quickly.
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
            // Write probe: exclusively create a unique file so diagnostics can never overwrite
            // destination data, including files left by older backup-sync versions.
            match run_with_policy(
                "cli::doctor write probe file",
                &io_policy,
                CancellationFlag::none(),
                || WritabilityProbe::create(&cfg.backup_root),
            ) {
                Ok(mut probe) => {
                    if let Err(error) = run_with_policy(
                        "cli::doctor remove probe file",
                        &io_policy,
                        CancellationFlag::none(),
                        || probe.remove(),
                    ) {
                        let correlation_id = cid("cli-doctor");
                        warn!(
                            cid = %correlation_id,
                            action = "probe_cleanup_failed",
                            probe_path = %probe.path()
                                .map(backup_core::logging::redact_path)
                                .unwrap_or_else(|| "<already-removed>".to_string()),
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

#[cfg(test)]
mod tests {
    use super::WritabilityProbe;

    #[test]
    fn writability_probe_preserves_legacy_probe_file() {
        let destination = tempfile::tempdir().expect("failed to create test destination");
        let legacy_probe = destination.path().join(".backup_sync_probe");
        let original = b"pre-existing destination data";
        std::fs::write(&legacy_probe, original).expect("failed to seed legacy probe file");

        let mut probe = WritabilityProbe::create(destination.path())
            .expect("failed to create exclusive writability probe");
        probe
            .remove()
            .expect("failed to clean up writability probe");

        assert_eq!(
            std::fs::read(&legacy_probe).expect("failed to read legacy probe file"),
            original
        );
        let probe_files = std::fs::read_dir(destination.path())
            .expect("failed to inspect test destination")
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(super::PROBE_PREFIX)
            })
            .count();
        assert_eq!(probe_files, 0, "temporary probe should be removed");
    }
}
