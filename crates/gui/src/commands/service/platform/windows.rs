use crate::commands::error::ErrorEnvelope;
use crate::commands::service::common::{
    run_command, run_command_with_retry, write_text_file, ServiceStatus,
};
use backup_core::service::windows_service;
use std::path::PathBuf;

/// Install a scheduled task for the daemon.
pub fn write_task(exec: &PathBuf) -> Result<PathBuf, ErrorEnvelope> {
    let dest = windows_service::default_task_xml_path().map_err(|e| {
        ErrorEnvelope::new(
            "SERVICE_PATH",
            format!(
                "service::windows::write_task failed to resolve task XML path: {}",
                e
            ),
        )
    })?;
    let xml = windows_service::write_schtasks_xml(PathBuf::new().as_path(), exec).map_err(|e| {
        ErrorEnvelope::new(
            "SERVICE_WRITE",
            format!(
                "service::windows::write_task failed to generate task XML: {}",
                e
            ),
        )
    })?;
    write_text_file(&dest, &xml, "write scheduled task XML")?;
    Ok(dest)
}

/// Register the daemon task for automatic execution.
pub fn enable_task(dest: &PathBuf) -> Result<(), ErrorEnvelope> {
    let dest_str = dest.to_str().ok_or_else(|| {
        ErrorEnvelope::new(
            "SERVICE_PATH",
            format!(
                "service::windows::enable_task invalid task XML path encoding: {:?}",
                dest
            ),
        )
    })?;
    run_command_with_retry(
        "schtasks",
        &["/Create", "/TN", "BackupSync", "/XML", dest_str, "/F"],
        "SERVICE_ENABLE",
        "schtasks /Create",
    )
}

/// Surface service health and fixes in the UI.
pub fn status_windows(
    reachable: bool,
    uptime_secs: Option<i64>,
    last_ipc: Option<i64>,
) -> Result<ServiceStatus, ErrorEnvelope> {
    let path = windows_service::default_task_xml_path().map_err(|e| {
        ErrorEnvelope::new(
            "SERVICE_PATH",
            format!(
                "service::windows::status_windows failed to resolve task XML path: {}",
                e
            ),
        )
    })?;
    if path.exists() {
        return Ok(ServiceStatus {
            installed: true,
            reachable,
            message: if reachable {
                format!("Service running (task XML at {:?})", path)
            } else {
                format!(
                    "Start on login configured (task XML at {:?}) but daemon unreachable",
                    path
                )
            },
            fix_command: Some("schtasks /Run /TN \"BackupSync\"".into()),
            uptime_secs,
            last_ipc_ts: last_ipc,
        });
    }
    Ok(ServiceStatus {
        installed: false,
        reachable: false,
        message: format!("Start on login not installed. XML expected at {:?}", path),
        fix_command: Some("backup-sync install-service --enable".into()),
        uptime_secs: None,
        last_ipc_ts: None,
    })
}

/// Allow users to recover a stuck daemon.
pub fn restart_daemon() -> Result<String, ErrorEnvelope> {
    run_command(
        "schtasks",
        &["/Run", "/TN", "BackupSync"],
        "RESTART_FAILED",
        "schtasks /Run",
    )?;
    Ok("Scheduled task triggered via schtasks.".to_string())
}
