use crate::commands::error::ErrorEnvelope;
use crate::commands::service::common::ServiceStatus;

/// Purpose: Returns an unsupported error for install requests.
///
/// Inputs: none.
/// Outputs: an error envelope indicating unsupported platform.
/// Ties to: service install commands on unsupported platforms.
/// Side effects: None.
/// Why: provide a clear error when service install is unavailable.
pub fn install_service() -> Result<String, ErrorEnvelope> {
    Err(ErrorEnvelope::new(
        "SERVICE_UNSUPPORTED",
        "Service install not implemented on this platform",
    ))
}

/// Purpose: Returns a status payload for unsupported platforms.
///
/// Inputs: none.
/// Outputs: a status payload with unsupported message.
/// Ties to: GUI status checks on unsupported platforms.
/// Side effects: None.
/// Why: avoid failures while signaling unsupported behavior.
pub fn status() -> Result<ServiceStatus, ErrorEnvelope> {
    Ok(ServiceStatus {
        installed: false,
        reachable: false,
        message: "Service status not implemented on this platform".into(),
        fix_command: None,
        uptime_secs: None,
        last_ipc_ts: None,
    })
}

/// Purpose: Returns an unsupported error for restart requests.
///
/// Inputs: none.
/// Outputs: an error envelope indicating unsupported platform.
/// Ties to: GUI restart actions on unsupported platforms.
/// Side effects: None.
/// Why: provide a clear error when restart is unavailable.
pub fn restart_daemon() -> Result<String, ErrorEnvelope> {
    Err(ErrorEnvelope::new(
        "RESTART_UNSUPPORTED",
        "Restart not implemented on this platform",
    ))
}
