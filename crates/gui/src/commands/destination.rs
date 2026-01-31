//! Validates backup destination paths without performing side-effectful writes.
use crate::commands::error::ErrorEnvelope;
use fs2::free_space;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize)]
/// Purpose: Result payload describing destination validation status.
///
/// Inputs: derived from filesystem checks.
/// Outputs: a serializable status structure.
/// Ties to: GUI destination pickers.
/// Side effects: None.
/// Why: communicate validation feedback to the frontend.
pub struct DestinationCheck {
    pub writable: bool,
    pub free_bytes: Option<u64>,
    pub message: String,
}

#[tauri::command]
/// Purpose: Validates a destination path without writing to disk.
///
/// Inputs: the destination path string.
/// Outputs: a `DestinationCheck` payload.
/// Ties to: GUI destination validation flows.
/// Side effects: Reads filesystem metadata and free space statistics.
/// Why: preflight destination settings before saving config.
pub fn check_destination_cmd(path: String) -> Result<DestinationCheck, ErrorEnvelope> {
    if path.trim().is_empty() {
        return Ok(DestinationCheck {
            writable: false,
            free_bytes: None,
            message: "Pick a backup destination.".into(),
        });
    }
    let p = PathBuf::from(path);
    if p.parent().is_none() {
        return Ok(DestinationCheck {
            writable: false,
            free_bytes: None,
            message: "Cannot use filesystem root. Pick a folder inside your home directory.".into(),
        });
    }
    if p.exists() && p.is_file() {
        return Ok(DestinationCheck {
            writable: false,
            free_bytes: None,
            message: "Destination points to a file; choose a folder.".into(),
        });
    }
    if !p.exists() {
        return Ok(DestinationCheck {
            writable: false,
            free_bytes: None,
            message: "Destination folder does not exist. Create it first, then select it.".into(),
        });
    }
    if !p.is_dir() {
        return Ok(DestinationCheck {
            writable: false,
            free_bytes: None,
            message: "Destination must be a folder.".into(),
        });
    }
    match free_space(&p) {
        Ok(free) => Ok(DestinationCheck {
            writable: true,
            free_bytes: Some(free),
            message: format!("Writable. Free space: {} bytes", free),
        }),
        Err(e) => Ok(DestinationCheck {
            writable: false,
            free_bytes: None,
            message: format!("Cannot read free space: {}", e),
        }),
    }
}
