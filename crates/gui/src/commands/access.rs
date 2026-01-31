use crate::commands::error::ErrorEnvelope;
use backup_core::load_config;
use fs2::free_space;
use serde::Serialize;
use std::fs;

#[derive(Serialize)]
/// Purpose: Access check payload for watched paths and destination.
///
/// Inputs: derived from filesystem probes.
/// Outputs: a structured access report.
/// Ties to: GUI access diagnostics.
/// Side effects: None.
/// Why: surface permission and existence issues to the user.
pub struct AccessProbe {
    pub destination_writable: bool,
    pub destination_message: String,
    pub watched_ok: Vec<String>,
    pub watched_missing: Vec<String>,
    pub watched_unwritable: Vec<String>,
}

#[tauri::command]
/// Purpose: Tests filesystem access for watched paths and destination.
///
/// Inputs: none.
/// Outputs: an `AccessProbe` or an error envelope.
/// Ties to: GUI diagnostics and support workflows.
/// Side effects: Reads config and filesystem metadata for watched paths.
/// Why: detect permissions and missing paths quickly.
pub fn test_access_cmd() -> Result<AccessProbe, ErrorEnvelope> {
    let cfg = load_config().map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_LOAD",
            format!("access::test_access_cmd failed to load config: {}", e),
        )
    })?;
    let (watched_ok, watched_missing, watched_unwritable) = classify_watched(&cfg);
    let (dest_ok, dest_msg) = probe_destination(&cfg.backup_root);

    Ok(AccessProbe {
        destination_writable: dest_ok,
        destination_message: dest_msg,
        watched_ok,
        watched_missing,
        watched_unwritable,
    })
}

/// Purpose: Classifies watched paths by existence and readability.
///
/// Inputs: the loaded config.
/// Outputs: three lists: ok, missing, and unwritable paths.
/// Ties to: access probing for GUI diagnostics.
/// Side effects: Reads filesystem metadata for watched paths.
/// Why: provide precise feedback on watched path issues.
fn classify_watched(cfg: &backup_core::Config) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut watched_ok = Vec::new();
    let mut watched_missing = Vec::new();
    let mut watched_unwritable = Vec::new();
    for w in &cfg.watched {
        if !w.path.exists() {
            watched_missing.push(w.path.display().to_string());
            continue;
        }
        if let Err(e) = fs::metadata(&w.path) {
            watched_unwritable.push(format!("{} (metadata failed: {})", w.path.display(), e));
            continue;
        }
        watched_ok.push(w.path.display().to_string());
    }
    (watched_ok, watched_missing, watched_unwritable)
}

/// Purpose: Probes the destination path for writability and free space.
///
/// Inputs: the destination path.
/// Outputs: a tuple of (is_writable, message).
/// Ties to: access probing for GUI diagnostics.
/// Side effects: Creates destination directories and reads free space metadata.
/// Why: surface destination issues with actionable messaging.
fn probe_destination(dest: &std::path::Path) -> (bool, String) {
    let dest_msg = if dest.as_os_str().is_empty() {
        "No backup destination set".to_string()
    } else if dest.exists() && dest.is_file() {
        format!("{:?} is a file; choose a folder", dest)
    } else {
        match fs::create_dir_all(dest) {
            Ok(_) => match free_space(dest) {
                Ok(free) => format!("Writable. Free space: {} bytes", free),
                Err(e) => format!("Writable, but failed to read free space: {}", e),
            },
            Err(e) => format!("Not writable: {}", e),
        }
    };
    (dest_msg.starts_with("Writable"), dest_msg)
}
