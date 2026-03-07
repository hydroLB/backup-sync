use crate::commands::error::ErrorEnvelope;
use crate::commands::service::common::{
    run_command, run_command_with_retry, write_text_file, ServiceStatus,
};
use backup_core::service::windows_service;
use std::path::PathBuf;

/// Summary: Writes the scheduled task XML to disk.
///
/// Inputs: the executable path.
///
/// Outputs: the destination XML path.
///
/// Side effects: Writes the scheduled task XML to disk.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: service installation on Windows.
///
/// Why this exists: install a scheduled task for the daemon.
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

/// Summary: Enables the scheduled task using schtasks.
///
/// Inputs: the task XML path.
///
/// Outputs: `Ok(())` when registration succeeds.
///
/// Side effects: Runs schtasks to register the scheduled task.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: service installation on Windows.
///
/// Why this exists: register the daemon task for automatic execution.
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

/// Summary: Builds the service status payload for Windows.
///
/// Inputs: daemon reachability and runtime metadata.
///
/// Outputs: a populated `ServiceStatus`.
///
/// Side effects: Reads filesystem metadata to check task XML presence.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI status reporting.
///
/// Why this exists: surface service health and fixes in the UI.
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

/// Summary: Restarts the daemon by triggering the scheduled task.
///
/// Inputs: none.
///
/// Outputs: a success message string.
///
/// Side effects: Runs schtasks to trigger the scheduled task.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI restart actions on Windows.
///
/// Why this exists: allow users to recover a stuck daemon.
pub fn restart_daemon() -> Result<String, ErrorEnvelope> {
    run_command(
        "schtasks",
        &["/Run", "/TN", "BackupSync"],
        "RESTART_FAILED",
        "schtasks /Run",
    )?;
    Ok("Scheduled task triggered via schtasks.".to_string())
}
