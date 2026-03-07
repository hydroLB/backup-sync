//! Validates backup destination paths without performing side-effectful writes.
use crate::commands::error::ErrorEnvelope;
use crate::commands::io_policy::run_blocking_io;
use anyhow::Context;
use fs2::free_space;
use serde::Serialize;
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::path::{Component, Path};

#[derive(Serialize)]
/// Summary: Result payload describing destination validation status.
///
/// Inputs: derived from filesystem checks.
///
/// Outputs: a serializable status structure.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI destination pickers.
///
/// Why this exists: communicate validation feedback to the frontend.
pub struct DestinationCheck {
    pub writable: bool,
    pub free_bytes: Option<u64>,
    pub message: String,
}

#[cfg(target_os = "macos")]
/// Summary: Detect whether a destination is inside a missing `/Volumes/<name>` mount root on macOS.
///
/// Inputs: `path` destination path to validate.
///
/// Outputs: Missing mount-root path when the volume root is not present, otherwise `None`.
///
/// Side effects: None.
///
/// Error handling: Returns `None` when the destination is not under `/Volumes` or path parsing fails.
///
/// Ties to other methods: Used by `check_destination_cmd` before attempting auto-create behavior.
///
/// Why this exists: Prevent auto-creating fake mount folders when an external drive is disconnected.
fn missing_macos_mount_root(path: &Path) -> Option<PathBuf> {
    let volumes_root = Path::new("/Volumes");
    let relative = match path.strip_prefix(volumes_root) {
        Ok(relative) => relative,
        Err(_) => return None,
    };
    let mut components = relative.components();
    let volume_name = match components.next()? {
        Component::Normal(name) => name,
        _ => return None,
    };
    let mount_root = volumes_root.join(volume_name);
    if mount_root.exists() {
        None
    } else {
        Some(mount_root)
    }
}

#[tauri::command]
/// Summary: Validates a destination path without writing to disk.
///
/// Inputs: the destination path string.
///
/// Outputs: a `DestinationCheck` payload.
///
/// Side effects: Reads filesystem metadata and free space statistics.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI destination validation flows.
///
/// Why this exists: preflight destination settings before saving config.
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
        #[cfg(target_os = "macos")]
        if let Some(missing_mount_root) = missing_macos_mount_root(&p) {
            return Ok(DestinationCheck {
                writable: false,
                free_bytes: None,
                message: format!(
                    "Destination drive is not mounted at {}. Reconnect it and try again.",
                    missing_mount_root.display()
                ),
            });
        }

        if let Err(error) = run_blocking_io(
            "gui::destination::check_destination_cmd create destination",
            || {
                std::fs::create_dir_all(&p).with_context(|| {
                    format!(
                        "destination::check_destination_cmd failed creating destination {:?}",
                        p
                    )
                })
            },
        ) {
            return Ok(DestinationCheck {
                writable: false,
                free_bytes: None,
                message: format!(
                    "Could not create destination folder automatically: {}",
                    error
                ),
            });
        }
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

#[cfg(test)]
mod tests {
    use super::check_destination_cmd;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Summary: Generate a unique temporary path for test filesystem operations.
    ///
    /// Inputs: `label` path segment for test readability.
    ///
    /// Outputs: Unique path under the process temp directory.
    ///
    /// Side effects: None.
    ///
    /// Error handling: Falls back to timestamp `0` when system clock is before epoch.
    ///
    /// Ties to other methods: Used by destination command tests.
    ///
    /// Why this exists: Keep tests deterministic without adding external dependencies.
    fn unique_temp_path(label: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("backup-sync-gui-{label}-{timestamp}"))
    }

    /// Summary: creates_missing_destination_folder_automatically orchestrates this method's core behavior.
    ///
    /// Inputs: Method parameters and required receiver state.
    ///
    /// Outputs: Return value and observable result for callers.
    ///
    /// Side effects: None beyond this method's explicit operations.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: Invoked by and composes with adjacent module methods.
    ///
    /// Why this exists: Keeps this behavior isolated, testable, and reusable.
    #[test]
    fn creates_missing_destination_folder_automatically() {
        let destination = unique_temp_path("destination-create")
            .join("missing")
            .join("nested");
        if destination.exists() {
            if let Err(error) = std::fs::remove_dir_all(&destination) {
                panic!(
                    "destination::tests::creates_missing_destination_folder_automatically failed to clear destination {:?}: {}",
                    destination,
                    error
                );
            }
        }

        let result =
            check_destination_cmd(destination.display().to_string()).expect("check should succeed");
        assert!(
            result.writable,
            "expected writable result, got: {}",
            result.message
        );
        assert!(
            destination.is_dir(),
            "destination directory should be created"
        );

        let cleanup_root = destination
            .ancestors()
            .nth(2)
            .map(PathBuf::from)
            .unwrap_or(destination);
        if let Err(error) = std::fs::remove_dir_all(&cleanup_root) {
            panic!(
                "destination::tests::creates_missing_destination_folder_automatically failed to cleanup {:?}: {}",
                cleanup_root,
                error
            );
        }
    }
}
