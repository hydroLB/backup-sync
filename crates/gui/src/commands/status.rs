use crate::api::status_api;
use crate::commands::correlation;
use backup_core::ActivityItem;
use serde::Serialize;

#[derive(Serialize)]
/// Decouple GUI payloads from IPC structs.
pub struct StatusDto {
    pub last_run_ts: Option<i64>,
    pub last_files_backed_up: usize,
    pub last_error: Option<String>,
    pub last_dirty_count: usize,
    pub last_safety_warning: Option<backup_core::SafetyWarning>,
    pub uptime_secs: Option<i64>,
    pub version: Option<String>,
    pub free_bytes: Option<u64>,
    pub last_verify_ts: Option<i64>,
    pub last_verify_status: Option<String>,
    pub last_verify_issues: Option<usize>,
    pub recent_activity: Vec<ActivityItem>,
    pub safe_mode: bool,
    pub destination_paused: bool,
    pub destination_pause_reason: Option<String>,
    pub destination_unavailable_ids: Vec<String>,
    pub destination_last_unavailable_ts: Option<i64>,
    pub destination_last_recovered_ts: Option<i64>,
    pub replication_last_run_ts: Option<i64>,
    pub replication_last_status: Option<String>,
    pub replication_last_error: Option<String>,
    pub replication_last_bytes_copied: u64,
    pub replication_last_blobs_copied: usize,
    pub replication_last_manifests_copied: usize,
    pub replication_last_manifests_deleted: usize,
    pub replication_last_pairs_ok: usize,
    pub replication_last_pairs_failed: usize,
    pub replication_last_targets_failed: Vec<String>,
    pub destinations: Vec<DestinationDto>,
}

#[derive(Serialize)]
/// Provide a stable schema for UI rendering.
pub struct DestinationDto {
    pub id: String,
    pub label: Option<String>,
    pub path: std::path::PathBuf,
    pub reachable: bool,
    pub writable: bool,
    pub free_bytes: Option<u64>,
    pub message: String,
}

impl From<status_api::Status> for StatusDto {
    /// Keep IPC structs separate from GUI payloads.
    fn from(s: status_api::Status) -> Self {
        Self {
            last_run_ts: s.last_run_ts,
            last_files_backed_up: s.last_files_backed_up,
            last_error: s.last_error,
            last_dirty_count: s.last_dirty_count,
            last_safety_warning: s.last_safety_warning,
            uptime_secs: s.uptime_secs,
            version: s.version,
            free_bytes: s.free_bytes,
            last_verify_ts: s.last_verify_ts,
            last_verify_status: s.last_verify_status,
            last_verify_issues: s.last_verify_issues,
            recent_activity: s.recent_activity,
            safe_mode: s.safe_mode,
            destination_paused: s.destination_paused,
            destination_pause_reason: s.destination_pause_reason,
            destination_unavailable_ids: s.destination_unavailable_ids,
            destination_last_unavailable_ts: s.destination_last_unavailable_ts,
            destination_last_recovered_ts: s.destination_last_recovered_ts,
            replication_last_run_ts: s.replication_last_run_ts,
            replication_last_status: s.replication_last_status,
            replication_last_error: s.replication_last_error,
            replication_last_bytes_copied: s.replication_last_bytes_copied,
            replication_last_blobs_copied: s.replication_last_blobs_copied,
            replication_last_manifests_copied: s.replication_last_manifests_copied,
            replication_last_manifests_deleted: s.replication_last_manifests_deleted,
            replication_last_pairs_ok: s.replication_last_pairs_ok,
            replication_last_pairs_failed: s.replication_last_pairs_failed,
            replication_last_targets_failed: s.replication_last_targets_failed,
            destinations: s
                .destinations
                .into_iter()
                .map(|d| DestinationDto {
                    id: d.id,
                    label: d.label,
                    path: d.path,
                    reachable: d.reachable,
                    writable: d.writable,
                    free_bytes: d.free_bytes,
                    message: d.message,
                })
                .collect(),
        }
    }
}

use crate::commands::error::ErrorEnvelope;
#[tauri::command]
/// Provide the UI with up to date daemon health information.
pub async fn get_status() -> Result<StatusDto, ErrorEnvelope> {
    let cid = correlation::cid("status", None);
    match status_api::fetch_status_with_correlation(Some(cid.as_str())).await {
        Ok(s) => Ok(StatusDto::from(s)),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("daemon IPC socket not found") {
                return Err(ErrorEnvelope::new(
                    "DAEMON_OFFLINE",
                    "Daemon not running yet. Finish setup, then enable Start on login to keep it running in the background.",
                ));
            }
            Err(ErrorEnvelope::from_anyhow_with_code(
                "STATUS_UNAVAILABLE",
                "status::get_status failed to fetch status",
                &e,
            ))
        }
    }
}
