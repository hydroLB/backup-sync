use crate::io::{run_with_policy, BlockingIoPolicy, CancellationFlag};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Summary: Resolves the systemd unit path for user or system scope.
///
/// Inputs: a user scope flag.
///
/// Outputs: the resolved unit file path.
///
/// Side effects: Reads the user config directory when user scope is requested.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: CLI and GUI service installation flows.
///
/// Why this exists: centralize systemd path decisions for consistent installs.
pub fn default_unit_path(user: bool) -> Result<PathBuf> {
    if user {
        let mut path = dirs::config_dir().ok_or_else(|| anyhow::anyhow!("config dir not found"))?;
        path.push("systemd/user/backup-sync.service");
        Ok(path)
    } else {
        Ok(PathBuf::from("/etc/systemd/system/backup-sync.service"))
    }
}

/// Summary: Builds and optionally writes a systemd unit file.
///
/// Inputs: the destination path, executable path, user scope, and optional log path.
///
/// Outputs: the unit file content string.
///
/// Side effects: Creates directories and writes the unit file when a destination is provided.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: service installation and manifest printing commands.
///
/// Why this exists: keep service unit formatting consistent across installation paths.
pub fn write_unit(
    destination: &Path,
    exec: &Path,
    user: bool,
    log_path: Option<&Path>,
) -> Result<String> {
    let io_policy = BlockingIoPolicy::bootstrap_defaults();
    let exec_str = exec.display().to_string();
    let log_str = log_path
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "/var/log/backup_sync.log".to_string());
    let after = if user {
        "default.target"
    } else {
        "network.target"
    };
    let contents = format!(
        "[Unit]
Description=Backup Sync Daemon
After={after}

[Service]
Type=simple
ExecStart={exec}
Restart=on-failure
StandardOutput=append:{log}
StandardError=append:{log}

[Install]
WantedBy=default.target
",
        exec = exec_str,
        log = log_str,
        after = after
    );

    if destination.as_os_str().is_empty() {
        return Ok(contents);
    }
    if let Some(parent) = destination.parent() {
        run_with_policy(
            "backup_core::service::systemd::write_unit create parent directory",
            &io_policy,
            CancellationFlag::none(),
            || {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create systemd directory {:?}", parent))
            },
        )?;
    }
    run_with_policy(
        "backup_core::service::systemd::write_unit write unit",
        &io_policy,
        CancellationFlag::none(),
        || {
            fs::write(destination, &contents)
                .with_context(|| format!("failed to write systemd unit to {:?}", destination))
        },
    )?;
    Ok(contents)
}
