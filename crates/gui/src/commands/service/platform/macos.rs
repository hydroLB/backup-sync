use crate::commands::error::ErrorEnvelope;
use crate::commands::service::common::{
    run_command, run_command_with_retry, write_text_file, ServiceStatus,
};
use daemon::integration::launchd;
use std::path::{Path, PathBuf};

const LAUNCHD_LABEL: &str = "com.backup_sync.daemon";

/// Purpose: Resolve the launchctl target identifier for the current user session.
///
/// Inputs: none.
/// Outputs: a `launchctl` domain-qualified service identifier.
/// Ties to: `restart_daemon` and best-effort daemon kickstarts after install.
/// Side effects: Executes `id -u` to determine the numeric UID.
/// Why: `launchctl kickstart` requires an explicit domain on modern macOS.
fn launchctl_target() -> Result<String, ErrorEnvelope> {
    let out = run_command("id", &["-u"], "SERVICE_UID", "id -u")?;
    let uid = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if uid.is_empty() {
        return Err(ErrorEnvelope::new(
            "SERVICE_UID",
            "service::macos::launchctl_target failed to parse UID from `id -u` output",
        ));
    }
    Ok(format!("gui/{}/{}", uid, LAUNCHD_LABEL))
}

/// Purpose: Writes the launchd plist to disk.
///
/// Inputs: the executable path and optional log path.
/// Outputs: the destination plist path.
/// Ties to: service installation on macOS.
/// Side effects: Writes the launchd plist file to disk.
/// Why: install the daemon to start on login.
pub fn write_plist(exec: &Path, log_path: Option<&Path>) -> Result<PathBuf, ErrorEnvelope> {
    let dest = launchd::default_plist_path().map_err(|e| {
        ErrorEnvelope::new(
            "SERVICE_PATH",
            format!(
                "service::macos::write_plist failed to resolve launchd path: {}",
                e
            ),
        )
    })?;
    let plist = launchd::write_plist(PathBuf::new().as_path(), exec, log_path).map_err(|e| {
        ErrorEnvelope::new(
            "SERVICE_WRITE",
            format!(
                "service::macos::write_plist failed to generate plist: {}",
                e
            ),
        )
    })?;
    write_text_file(&dest, &plist, "write launchd plist")?;
    Ok(dest)
}

/// Purpose: Enables the launchd service using launchctl.
///
/// Inputs: the plist path.
/// Outputs: `Ok(())` when launchctl load succeeds.
/// Ties to: service installation on macOS.
/// Side effects: Runs launchctl to load the plist.
/// Why: register the daemon with launchd for startup.
pub fn enable_launchd(dest: &PathBuf) -> Result<(), ErrorEnvelope> {
    let dest_str = dest.to_str().ok_or_else(|| {
        ErrorEnvelope::new(
            "SERVICE_PATH",
            format!(
                "service::macos::enable_launchd invalid plist path encoding: {:?}",
                dest
            ),
        )
    })?;
    run_command_with_retry(
        "launchctl",
        &["load", dest_str],
        "SERVICE_ENABLE",
        "launchctl load",
    )?;

    // Best-effort kickstart, but do not fail the install if it is not supported.
    if let Ok(target) = launchctl_target() {
        let _ = run_command(
            "launchctl",
            &["kickstart", "-k", &target],
            "SERVICE_ENABLE",
            "launchctl kickstart",
        );
    }
    Ok(())
}

/// Purpose: Builds the service status payload for macOS.
///
/// Inputs: daemon reachability and runtime metadata.
/// Outputs: a populated `ServiceStatus`.
/// Ties to: GUI status reporting.
/// Side effects: Reads filesystem metadata to check plist presence.
/// Why: surface service health and fixes in the UI.
pub fn status_macos(
    reachable: bool,
    uptime_secs: Option<i64>,
    last_ipc: Option<i64>,
) -> Result<ServiceStatus, ErrorEnvelope> {
    let path = launchd::default_plist_path().map_err(|e| {
        ErrorEnvelope::new(
            "SERVICE_PATH",
            format!(
                "service::macos::status_macos failed to resolve launchd path: {}",
                e
            ),
        )
    })?;
    if path.exists() {
        return Ok(ServiceStatus {
            installed: true,
            reachable,
            message: if reachable {
                format!("Service running (launchd plist at {:?})", path)
            } else {
                format!(
                    "Start on login enabled (launchd plist at {:?}) but daemon unreachable",
                    path
                )
            },
            fix_command: Some(format!("launchctl load {:?}", path)),
            uptime_secs,
            last_ipc_ts: last_ipc,
        });
    }
    Ok(ServiceStatus {
        installed: false,
        reachable: false,
        message: format!("Start on login not installed. Plist expected at {:?}", path),
        fix_command: Some(
            "Use tray: Start on login, or run: backup-sync install-service --enable".into(),
        ),
        uptime_secs: None,
        last_ipc_ts: None,
    })
}

/// Purpose: Restarts the daemon using launchctl.
///
/// Inputs: none.
/// Outputs: a success message string.
/// Ties to: GUI restart actions on macOS.
/// Side effects: Runs launchctl to restart the daemon.
/// Why: allow users to recover a stuck daemon.
pub fn restart_daemon() -> Result<String, ErrorEnvelope> {
    let target = launchctl_target().unwrap_or_else(|_| LAUNCHD_LABEL.to_string());
    run_command(
        "launchctl",
        &["kickstart", "-k", &target],
        "RESTART_FAILED",
        "launchctl kickstart",
    )?;
    Ok(format!("Daemon restarted via launchctl ({target})."))
}
