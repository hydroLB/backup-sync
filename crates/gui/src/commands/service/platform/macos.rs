use crate::commands::error::ErrorEnvelope;
use crate::commands::service::common::{
    run_command, run_command_with_retry, write_text_file, ServiceStatus,
};
use backup_core::service::launchd;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use tracing::warn;

const LAUNCHD_LABEL: &str = "com.backup_sync.daemon";

/// `launchctl kickstart` requires an explicit domain on modern macOS.
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

/// Install the daemon to start on login.
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

/// Register the daemon with launchd for startup.
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
        if let Err(error) = run_command(
            "launchctl",
            &["kickstart", "-k", &target],
            "SERVICE_ENABLE",
            "launchctl kickstart",
        ) {
            warn!(
                "service::macos::enable_launchd launchctl kickstart failed (non-fatal): {}",
                error.message
            );
        }
    }
    Ok(())
}

/// Surface service health and fixes in the UI.
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

/// Allow users to recover a stuck daemon.
pub fn restart_daemon() -> Result<String, ErrorEnvelope> {
    let target = match launchctl_target() {
        Ok(target) => target,
        Err(error) => {
            warn!(
                "service::macos::restart_daemon falling back to default launchd label due to target resolution failure: {}",
                error.message
            );
            LAUNCHD_LABEL.to_string()
        }
    };
    match run_command(
        "launchctl",
        &["kickstart", "-k", &target],
        "RESTART_FAILED",
        "launchctl kickstart",
    ) {
        Ok(_) => Ok(format!("Daemon restarted via launchctl ({target}).")),
        Err(error) => {
            warn!(
                "service::macos::restart_daemon launchctl restart failed; falling back to direct daemon restart: {}",
                error.message
            );
            restart_daemon_direct()
        }
    }
}

/// Development launches and non-launchd sessions still need a reliable daemon refresh path after config changes.
fn restart_daemon_direct() -> Result<String, ErrorEnvelope> {
    let (exec, log_path) = crate::commands::service::common::load_exec_and_log()?;
    terminate_matching_daemons(&exec)?;
    spawn_daemon_process(&exec, log_path.as_deref())?;
    Ok(format!("Daemon restarted directly via {:?}.", exec))
}

/// Prevent duplicate daemons from racing for the same IPC socket or state files.
fn terminate_matching_daemons(exec: &Path) -> Result<(), ErrorEnvelope> {
    let exec_str = exec.to_str().ok_or_else(|| {
        ErrorEnvelope::new(
            "RESTART_FAILED",
            format!(
                "service::macos::terminate_matching_daemons invalid executable path encoding: {:?}",
                exec
            ),
        )
    })?;
    let output = Command::new("pgrep")
        .args(["-f", exec_str])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| {
            ErrorEnvelope::new(
                "RESTART_FAILED",
                format!(
                    "service::macos::terminate_matching_daemons failed to run pgrep for {:?}: {}",
                    exec, error
                ),
            )
        })?;
    if !output.status.success() {
        return Ok(());
    }
    let pids: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    for pid in &pids {
        let status = Command::new("kill")
            .args(["-TERM", pid.as_str()])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .status()
            .map_err(|error| {
                ErrorEnvelope::new(
                    "RESTART_FAILED",
                    format!(
                        "service::macos::terminate_matching_daemons failed to signal pid {} for {:?}: {}",
                        pid, exec, error
                    ),
                )
            })?;
        if !status.success() {
            warn!(
                "service::macos::terminate_matching_daemons failed to terminate pid {} for {:?}",
                pid, exec
            );
        }
    }
    wait_for_process_exit(exec, &pids, Duration::from_secs(2))?;
    Ok(())
}

/// The dev daemon can keep the IPC socket open briefly; waiting and escalating prevents the
/// replacement daemon from losing the bind race.
fn wait_for_process_exit(
    exec: &Path,
    pids: &[String],
    timeout: Duration,
) -> Result<(), ErrorEnvelope> {
    let start = std::time::Instant::now();
    loop {
        let alive = alive_process_ids(pids)?;
        if alive.is_empty() {
            return Ok(());
        }
        if start.elapsed() >= timeout {
            for pid in &alive {
                let status = Command::new("kill")
                    .args(["-KILL", pid])
                    .stdout(Stdio::null())
                    .stderr(Stdio::piped())
                    .status()
                    .map_err(|error| {
                        ErrorEnvelope::new(
                            "RESTART_FAILED",
                            format!(
                                "service::macos::wait_for_process_exit failed to force-kill pid {} for {:?}: {}",
                                pid, exec, error
                            ),
                        )
                    })?;
                if !status.success() {
                    return Err(ErrorEnvelope::new(
                        "RESTART_FAILED",
                        format!(
                            "service::macos::wait_for_process_exit failed to force-kill pid {} for {:?}",
                            pid, exec
                        ),
                    ));
                }
            }
            thread::sleep(Duration::from_millis(200));
            let stubborn = alive_process_ids(pids)?;
            if stubborn.is_empty() {
                return Ok(());
            }
            return Err(ErrorEnvelope::new(
                "RESTART_FAILED",
                format!(
                    "service::macos::wait_for_process_exit daemon pids still alive after escalation for {:?}: {}",
                    exec,
                    stubborn.join(", ")
                ),
            ));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// Keep the restart wait loop readable and avoid duplicating liveness probe collection logic.
fn alive_process_ids(pids: &[String]) -> Result<Vec<String>, ErrorEnvelope> {
    let mut alive = Vec::new();
    for pid in pids {
        if process_is_alive(pid)?.is_some() {
            alive.push(pid.clone());
        }
    }
    Ok(alive)
}

/// Direct daemon restarts need a portable way to confirm the old process released the IPC socket
/// before spawning the replacement.
fn process_is_alive(pid: &str) -> Result<Option<&str>, ErrorEnvelope> {
    let stat_output = Command::new("ps")
        .args(["-p", pid, "-o", "stat="])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .map_err(|error| {
            ErrorEnvelope::new(
                "RESTART_FAILED",
                format!(
                    "service::macos::process_is_alive failed to inspect pid {} state: {}",
                    pid, error
                ),
            )
        })?;
    if stat_output.status.success() {
        let stat = String::from_utf8_lossy(&stat_output.stdout)
            .trim()
            .to_string();
        if stat.starts_with('Z') {
            return Ok(None);
        }
    }
    let status = Command::new("kill")
        .args(["-0", pid])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| {
            ErrorEnvelope::new(
                "RESTART_FAILED",
                format!(
                    "service::macos::process_is_alive failed to probe pid {}: {}",
                    pid, error
                ),
            )
        })?;
    if status.success() {
        Ok(Some(pid))
    } else {
        Ok(None)
    }
}

/// Config saves must take effect immediately even when the daemon was started manually during development.
fn spawn_daemon_process(exec: &Path, log_path: Option<&Path>) -> Result<(), ErrorEnvelope> {
    let config_path = backup_core::platform::paths::config_file_path().map_err(|error| {
        ErrorEnvelope::new(
            "RESTART_FAILED",
            format!(
                "service::macos::spawn_daemon_process failed to resolve config path: {}",
                error
            ),
        )
    })?;
    let mut command = Command::new(exec);
    command
        .env("BACKUP_SYNC_CONFIG", &config_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(path) = log_path {
        command.env("BACKUP_SYNC_LOG", path);
    }
    command.spawn().map_err(|error| {
        ErrorEnvelope::new(
            "RESTART_FAILED",
            format!(
                "service::macos::spawn_daemon_process failed to start daemon {:?}: {}",
                exec, error
            ),
        )
    })?;
    Ok(())
}
