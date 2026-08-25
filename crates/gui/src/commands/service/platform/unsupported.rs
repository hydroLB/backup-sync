use crate::commands::error::ErrorEnvelope;
use crate::commands::service::common::ServiceStatus;

/// Provide a clear error when service install is unavailable.
pub fn install_service() -> Result<String, ErrorEnvelope> {
    Err(ErrorEnvelope::new(
        "SERVICE_UNSUPPORTED",
        "Service install not implemented on this platform",
    ))
}

/// Avoid failures while signaling unsupported behavior.
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

/// Provide a clear error when restart is unavailable.
pub fn restart_daemon() -> Result<String, ErrorEnvelope> {
    Err(ErrorEnvelope::new(
        "RESTART_UNSUPPORTED",
        "Restart not implemented on this platform",
    ))
}
