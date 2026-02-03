use crate::api::status_api;
use backup_core::ActivityItem;
use serde::Serialize;

#[derive(Serialize)]
/// Purpose: Serializable status payload for the GUI.
///
/// Inputs: derived from IPC status responses.
/// Outputs: a frontend friendly status object.
/// Ties to: GUI status panels and IPC status translation.
/// Side effects: None.
/// Why: decouple GUI payloads from IPC structs.
pub struct StatusDto {
    pub last_run_ts: Option<i64>,
    pub last_files_backed_up: usize,
    pub last_error: Option<String>,
    pub last_dirty_count: usize,
    pub uptime_secs: Option<i64>,
    pub version: Option<String>,
    pub free_bytes: Option<u64>,
    pub last_verify_ts: Option<i64>,
    pub last_verify_status: Option<String>,
    pub last_verify_issues: Option<usize>,
    pub recent_activity: Vec<ActivityItem>,
    pub safe_mode: bool,
    pub destinations: Vec<DestinationDto>,
}

#[derive(Serialize)]
/// Purpose: Serializable destination status payload for the GUI.
///
/// Inputs: derived from IPC destination status responses.
/// Outputs: a frontend friendly destination object.
/// Ties to: GUI status panels.
/// Side effects: None.
/// Why: provide a stable schema for UI rendering.
pub struct DestinationDto {
    pub id: String,
    pub label: Option<String>,
    pub path: std::path::PathBuf,
    pub free_bytes: Option<u64>,
}

impl From<status_api::Status> for StatusDto {
    /// Purpose: Converts IPC status into GUI status payloads.
    ///
    /// Inputs: a `status_api::Status` value.
    /// Outputs: a `StatusDto` value.
    /// Ties to: IPC response handling for GUI commands.
    /// Side effects: None.
    /// Why: keep IPC structs separate from GUI payloads.
    fn from(s: status_api::Status) -> Self {
        Self {
            last_run_ts: s.last_run_ts,
            last_files_backed_up: s.last_files_backed_up,
            last_error: s.last_error,
            last_dirty_count: s.last_dirty_count,
            uptime_secs: s.uptime_secs,
            version: s.version,
            free_bytes: s.free_bytes,
            last_verify_ts: s.last_verify_ts,
            last_verify_status: s.last_verify_status,
            last_verify_issues: s.last_verify_issues,
            recent_activity: s.recent_activity,
            safe_mode: s.safe_mode,
            destinations: s
                .destinations
                .into_iter()
                .map(|d| DestinationDto {
                    id: d.id,
                    label: d.label,
                    path: d.path,
                    free_bytes: d.free_bytes,
                })
                .collect(),
        }
    }
}

use crate::commands::error::ErrorEnvelope;
#[tauri::command]
/// Purpose: Fetches the current daemon status over IPC.
///
/// Inputs: none.
/// Outputs: a `StatusDto` or an error envelope.
/// Ties to: GUI status refresh actions.
/// Side effects: Performs an IPC request to the daemon.
/// Why: provide the UI with up to date daemon health information.
pub async fn get_status() -> Result<StatusDto, ErrorEnvelope> {
    match status_api::fetch_status().await {
        Ok(s) => Ok(StatusDto::from(s)),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("daemon IPC socket not found") {
                return Err(ErrorEnvelope::new(
                    "DAEMON_OFFLINE",
                    "Daemon not running yet. Finish setup, then enable Start on login to keep it running in the background.",
                ));
            }
            Err(ErrorEnvelope::new(
                "STATUS_UNAVAILABLE",
                format!("status::get_status failed to fetch status: {}", e),
            ))
        }
    }
}
