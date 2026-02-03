use crate::commands::error::ErrorEnvelope;
use crate::commands::status::get_status;
use crate::commands::{auth::SessionAuth, security};
use backup_core::{load_config, validate};
use tauri::State;

mod common;
mod platform;

pub use common::ServiceStatus;

/// Purpose: Installs the platform service for start on login.
///
/// Inputs: an optional correlation id and session auth state.
/// Outputs: a status message or an error envelope.
/// Ties to: GUI service installation actions.
/// Side effects: Writes service manifests and runs platform enable commands.
/// Why: enable background execution without manual terminal steps.
#[tauri::command]
pub async fn install_service_cmd(
    correlation_id: Option<String>,
    auth_state: State<'_, SessionAuth>,
) -> Result<String, ErrorEnvelope> {
    security::ensure_unlocked(&auth_state, correlation_id.clone())?;

    // Installing a service that immediately crash-loops due to missing/invalid config is noisy
    // and makes it harder to diagnose first-run issues.
    let cfg = load_config().map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_INVALID",
            format!(
                "service::install_service_cmd failed to load config; complete setup first: {}",
                e
            ),
        )
    })?;
    validate(&cfg).map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_INVALID",
            format!(
                "service::install_service_cmd config validation failed; fix config first: {}",
                e
            ),
        )
    })?;

    let (exec, log_path) = common::load_exec_and_log()?;
    let log_path = log_path.as_deref();
    #[cfg(target_os = "macos")]
    let message = {
        let dest = platform::macos::write_plist(&exec, log_path)?;
        platform::macos::enable_launchd(&dest)?;
        format!("Service installed at {:?}. Enabled via launchctl.", dest)
    };
    #[cfg(target_os = "linux")]
    let message = {
        let dest = platform::linux::write_unit(&exec, log_path)?;
        platform::linux::enable_systemd()?;
        format!("Service installed at {:?} and enabled via systemd.", dest)
    };
    #[cfg(target_os = "windows")]
    let message = {
        let dest = platform::windows::write_task(&exec)?;
        platform::windows::enable_task(&dest)?;
        format!(
            "Service installed at {:?} and registered via schtasks.",
            dest
        )
    };
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        return platform::unsupported::install_service();
    }
    Ok(message)
}

/// Purpose: Checks the service status and daemon reachability.
///
/// Inputs: none.
/// Outputs: a `ServiceStatus` payload or an error envelope.
/// Ties to: GUI status panels.
/// Side effects: Performs IPC to the daemon and reads service metadata.
/// Why: surface service health and potential fixes in the UI.
#[tauri::command]
pub async fn check_service_cmd() -> Result<ServiceStatus, ErrorEnvelope> {
    let status = get_status().await.ok();
    let reachable = status.is_some();
    let uptime = status.as_ref().and_then(|s| s.uptime_secs);
    let last_ipc = if reachable {
        Some(chrono::Utc::now().timestamp())
    } else {
        None
    };
    #[cfg(target_os = "macos")]
    let status = platform::macos::status_macos(reachable, uptime, last_ipc);
    #[cfg(target_os = "linux")]
    let status = platform::linux::status_linux(reachable, uptime, last_ipc);
    #[cfg(target_os = "windows")]
    let status = platform::windows::status_windows(reachable, uptime, last_ipc);
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let status = platform::unsupported::status();
    status
}

/// Purpose: Restarts the daemon using platform specific tools.
///
/// Inputs: an optional correlation id and session auth state.
/// Outputs: a status message or an error envelope.
/// Ties to: GUI restart actions.
/// Side effects: Runs platform restart commands for the daemon.
/// Why: allow users to recover the daemon without leaving the UI.
#[tauri::command]
pub fn restart_daemon_cmd(
    correlation_id: Option<String>,
    auth_state: State<'_, SessionAuth>,
) -> Result<String, ErrorEnvelope> {
    security::ensure_unlocked(&auth_state, correlation_id.clone())?;
    #[cfg(target_os = "macos")]
    let result = platform::macos::restart_daemon();
    #[cfg(target_os = "linux")]
    let result = platform::linux::restart_daemon();
    #[cfg(target_os = "windows")]
    let result = platform::windows::restart_daemon();
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let result = platform::unsupported::restart_daemon();
    result
}
