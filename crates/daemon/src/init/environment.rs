use anyhow::{Context, Result};
use backup_core::io::{run_with_policy, BlockingIoPolicy, CancellationFlag};
use backup_core::platform::paths;
use std::path::PathBuf;

/// Summary: Snapshot of resolved daemon paths after environment validation.
///
/// Inputs: resolved from platform specific path helpers.
///
/// Outputs: a set of resolved filesystem paths.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon startup diagnostics.
///
/// Why this exists: keep path resolution centralized and reusable.
#[derive(Debug, Clone)]
pub struct EnvironmentSnapshot {
    pub config_path: PathBuf,
    pub state_path: PathBuf,
    pub log_path: PathBuf,
}

/// Summary: Validates that required directories are available and writable.
///
/// Inputs: none.
///
/// Outputs: an `EnvironmentSnapshot` with resolved paths.
///
/// Side effects: Creates parent directories required for daemon persistence.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon startup checks before loading config or state.
///
/// Why this exists: fail fast when the environment cannot persist required files.
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

/// Summary: Ensures the parent directory for a file path exists.
///
/// Inputs: the file path and a label used in error messages.
///
/// Outputs: `Ok(())` when the directory exists or is created.
///
/// Side effects: Creates parent directories on disk when missing.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: environment validation for daemon persistence paths.
///
/// Why this exists: guarantee directories are ready before IO begins.
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
