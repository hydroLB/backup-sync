use crate::commands::status::get_status;
use chrono::Utc;

/// Purpose: Refreshes the system tray tooltip using the latest daemon status.
///
/// Inputs: the Tauri app handle.
/// Outputs: `()` after attempting to set the tooltip.
/// Ties to: tray refresh loops and IPC status calls.
/// Side effects: Performs IPC calls and updates the tray tooltip.
/// Why: keep tray feedback aligned with daemon health.
pub(crate) async fn update_tray_tooltip(handle: &tauri::AppHandle) {
    match get_status().await {
        Ok(status) => {
            let mut parts: Vec<String> = vec!["Online".into(), "Local-only".into()];
            if let Some(free) = status.free_bytes {
                let gb = (free as f64) / (1024.0 * 1024.0 * 1024.0);
                parts.push(format!("{:.1} GB free", gb));
                if free < 2 * 1024 * 1024 * 1024 {
                    parts.push("LOW SPACE".into());
                }
            }
            if status.safe_mode {
                parts.push("SAFE MODE".into());
            }
            if let Some(issues) = status.last_verify_issues {
                if issues > 0 {
                    parts.push("VERIFY ISSUES".into());
                }
            } else if let Some(ts) = status.last_verify_ts {
                let now = Utc::now().timestamp();
                let mins = (now - ts).max(0) / 60;
                if mins < 120 {
                    parts.push(format!("Verified {}m ago", mins));
                } else {
                    parts.push(format!("Verified {}h ago", mins / 60));
                }
            } else {
                parts.push("VERIFY PENDING".into());
            }
            let label = format!("Backup Sync • {}", parts.join(" • "));
            let _ = handle.tray_handle().set_tooltip(&label);
        }
        Err(_) => {
            let _ = handle.tray_handle().set_tooltip("Backup Sync • Offline");
        }
    }
}
