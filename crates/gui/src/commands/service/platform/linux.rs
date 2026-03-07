use crate::commands::error::ErrorEnvelope;
use crate::commands::service::common::{
    run_command, run_command_with_retry, write_text_file, ServiceStatus,
};
use backup_core::service::systemd;
use std::path::{Path, PathBuf};

/// Summary: Writes the systemd unit file to disk.
///
/// Inputs: the executable path and optional log path.
///
/// Outputs: the destination unit path.
///
/// Side effects: Writes the systemd unit file to disk.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: service installation on Linux.
///
/// Why this exists: install the daemon for user level startup.
pub fn write_unit(exec: &PathBuf, log_path: Option<&Path>) -> Result<PathBuf, ErrorEnvelope> {
    let dest = systemd::default_unit_path(true).map_err(|e| {
        ErrorEnvelope::new(
            "SERVICE_PATH",
            format!(
                "service::linux::write_unit failed to resolve systemd path: {}",
                e
            ),
        )
    })?;
    let unit =
        systemd::write_unit(PathBuf::new().as_path(), exec, true, log_path).map_err(|e| {
            ErrorEnvelope::new(
                "SERVICE_WRITE",
                format!(
                    "service::linux::write_unit failed to generate systemd unit: {}",
                    e
                ),
            )
        })?;
    write_text_file(&dest, &unit, "write systemd unit")?;
    Ok(dest)
}

/// Summary: Enables the systemd user service.
///
/// Inputs: none.
///
/// Outputs: `Ok(())` when systemctl enable succeeds.
///
/// Side effects: Runs systemctl commands to reload and enable services.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: service installation on Linux.
///
/// Why this exists: start the daemon automatically for the current user.
pub fn enable_systemd() -> Result<(), ErrorEnvelope> {
    run_command_with_retry(
        "systemctl",
        &["--user", "daemon-reload"],
        "SERVICE_ENABLE",
        "systemctl --user daemon-reload",
    )?;
    run_command(
        "systemctl",
        &["--user", "enable", "--now", "backup-sync.service"],
        "SERVICE_ENABLE",
        "systemctl --user enable --now",
    )?;
    Ok(())
}

/// Summary: Builds the service status payload for Linux.
///
/// Inputs: daemon reachability and runtime metadata.
///
/// Outputs: a populated `ServiceStatus`.
///
/// Side effects: Reads filesystem metadata to check unit presence.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI status reporting.
///
/// Why this exists: surface service health and fixes in the UI.
pub fn status_linux(
    reachable: bool,
    uptime_secs: Option<i64>,
    last_ipc: Option<i64>,
) -> Result<ServiceStatus, ErrorEnvelope> {
    let path = systemd::default_unit_path(true).map_err(|e| {
        ErrorEnvelope::new(
            "SERVICE_PATH",
            format!(
                "service::linux::status_linux failed to resolve systemd unit path: {}",
                e
            ),
        )
    })?;
    if path.exists() {
        return Ok(ServiceStatus {
            installed: true,
            reachable,
            message: if reachable {
                format!("Service running (systemd unit at {:?})", path)
            } else {
                format!(
                    "Start on login enabled (systemd unit at {:?}) but daemon unreachable",
                    path
                )
            },
            fix_command: Some("systemctl --user enable --now backup-sync.service".into()),
            uptime_secs,
            last_ipc_ts: last_ipc,
        });
    }
    Ok(ServiceStatus {
        installed: false,
        reachable: false,
        message: format!("Start on login not installed. Unit expected at {:?}", path),
        fix_command: Some("backup-sync install-service --enable".into()),
        uptime_secs: None,
        last_ipc_ts: None,
    })
}

/// Summary: Restarts the daemon using systemd.
///
/// Inputs: none.
///
/// Outputs: a success message string.
///
/// Side effects: Runs systemctl restart to restart the daemon.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI restart actions on Linux.
///
/// Why this exists: allow users to recover a stuck daemon.
pub fn restart_daemon() -> Result<String, ErrorEnvelope> {
    run_command(
        "systemctl",
        &["--user", "restart", "backup-sync.service"],
        "RESTART_FAILED",
        "systemctl --user restart",
    )?;
    Ok("Daemon restarted via systemctl --user.".to_string())
}
