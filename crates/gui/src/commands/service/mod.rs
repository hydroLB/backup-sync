use crate::commands::error::ErrorEnvelope;
use crate::commands::status::get_status;
use backup_core::load_validated_config;
use tracing::warn;

pub(crate) mod common;
mod platform;

pub use common::ServiceStatus;

/// Enable background execution without manual terminal steps.
#[tauri::command]
pub async fn install_service_cmd(_correlation_id: Option<String>) -> Result<String, ErrorEnvelope> {
    // Installing a service that immediately crash-loops due to missing/invalid config is noisy
    // and makes it harder to diagnose first-run issues.
    load_validated_config().map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_INVALID",
            format!(
                "service::install_service_cmd failed to load config; complete setup first: {}",
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

/// Surface service health and potential fixes in the UI.
#[tauri::command]
pub async fn check_service_cmd() -> Result<ServiceStatus, ErrorEnvelope> {
    let status = match get_status().await {
        Ok(status) => Some(status),
        Err(error) => {
            warn!(
                "service::check_service_cmd failed to fetch daemon status; continuing with unreachable state: {}",
                error.message
            );
            None
        }
    };
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

/// Allow users to recover the daemon without leaving the UI.
#[tauri::command]
pub fn restart_daemon_cmd(_correlation_id: Option<String>) -> Result<String, ErrorEnvelope> {
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
