use crate::api::logs_api;
use crate::commands::error::ErrorEnvelope;
use dirs::desktop_dir;

#[tauri::command]
/// Summary: Returns the most recent daemon log lines.
///
/// Inputs: none.
///
/// Outputs: a log tail string or an error envelope.
///
/// Side effects: Reads the daemon log file from disk.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI log tail requests.
///
/// Why this exists: provide quick access to recent log activity.
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
/// Summary: Exports the daemon logs to the Desktop directory.
///
/// Inputs: none.
///
/// Outputs: the path to the exported log file or an error envelope.
///
/// Side effects: Reads the log file and writes an exported copy to disk.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI export logs actions.
///
/// Why this exists: allow users to share logs for diagnostics.
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
