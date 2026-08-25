use crate::api::logs_api;
use crate::commands::error::ErrorEnvelope;
use dirs::desktop_dir;

#[tauri::command]
/// Provide quick access to recent log activity.
pub fn log_tail_cmd() -> Result<String, ErrorEnvelope> {
    logs_api::read_log_tail(None).map_err(|e| {
        ErrorEnvelope::from_anyhow_with_code(
            "LOG_TAIL",
            "logs::log_tail_cmd failed to read log tail",
            &e,
        )
    })
}

#[tauri::command]
/// Allow users to share logs for diagnostics.
pub fn export_logs_cmd() -> Result<String, ErrorEnvelope> {
    let dest_dir = desktop_dir()
        .ok_or_else(|| ErrorEnvelope::new("NO_DESKTOP", "No desktop directory available"))?;
    let dest = logs_api::export_logs(&dest_dir).map_err(|e| {
        ErrorEnvelope::from_anyhow_with_code(
            "LOG_EXPORT",
            "logs::export_logs_cmd failed to export logs",
            &e,
        )
    })?;
    Ok(dest.display().to_string())
}
