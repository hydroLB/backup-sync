use crate::commands::error::ErrorEnvelope;
use crate::commands::service::common::{
    run_command, run_command_with_retry, write_text_file, ServiceStatus,
};
use backup_core::service::systemd;
use std::path::{Path, PathBuf};

/// Install the daemon for user level startup.
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

/// Start the daemon automatically for the current user.
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

/// Surface service health and fixes in the UI.
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

/// Allow users to recover a stuck daemon.
pub fn restart_daemon() -> Result<String, ErrorEnvelope> {
    run_command(
        "systemctl",
        &["--user", "restart", "backup-sync.service"],
        "RESTART_FAILED",
        "systemctl --user restart",
    )?;
    Ok("Daemon restarted via systemctl --user.".to_string())
}
