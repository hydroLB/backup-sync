use anyhow::{Context, Result};
use backup_core::io::{run_with_policy, BlockingIoPolicy, CancellationFlag};
use backup_core::platform::paths;
use std::path::PathBuf;

/// Keep path resolution centralized and reusable.
#[derive(Debug, Clone)]
pub struct EnvironmentSnapshot {
    pub config_path: PathBuf,
    pub state_path: PathBuf,
    pub log_path: PathBuf,
}

/// Fail fast when the environment cannot persist required files.
pub fn validate_environment() -> Result<EnvironmentSnapshot> {
    let config_path = paths::config_file_path()
        .context("daemon::init::validate_environment failed to resolve config path")?;
    let state_path = paths::state_file_path()
        .context("daemon::init::validate_environment failed to resolve state path")?;
    let log_path = paths::log_file_path()
        .context("daemon::init::validate_environment failed to resolve log path")?;
    ensure_parent_dir(&config_path, "config")?;
    ensure_parent_dir(&state_path, "state")?;
    ensure_parent_dir(&log_path, "log")?;
    Ok(EnvironmentSnapshot {
        config_path,
        state_path,
        log_path,
    })
}

/// Guarantee directories are ready before IO begins.
pub fn ensure_parent_dir(path: &PathBuf, label: &str) -> Result<()> {
    let io_policy = BlockingIoPolicy::bootstrap_defaults();
    let parent = path.parent().ok_or_else(|| {
        anyhow::anyhow!(
            "daemon::init::ensure_parent_dir missing parent directory for {} path {:?}",
            label,
            path
        )
    })?;
    run_with_policy(
        "daemon::init::ensure_parent_dir create parent directory",
        &io_policy,
        CancellationFlag::none(),
        || {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "daemon::init::ensure_parent_dir failed to create {} parent directory {:?}",
                    label, parent
                )
            })
        },
    )?;
    Ok(())
}
