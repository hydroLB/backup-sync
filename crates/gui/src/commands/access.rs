use crate::commands::error::ErrorEnvelope;
use crate::commands::io_policy::run_blocking_io;
use anyhow::Context;
use backup_core::load_validated_config;
use fs2::free_space;
use serde::Serialize;
use std::fs;

#[derive(Serialize, Clone, Debug)]
/// Summary: Access check payload for watched paths and destination.
///
/// Inputs: derived from filesystem probes.
///
/// Outputs: a structured access report.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI access diagnostics.
///
/// Why this exists: surface permission and existence issues to the user.
pub struct AccessProbe {
    pub destination_writable: bool,
    pub destination_message: String,
    pub watched_ok: Vec<String>,
    pub watched_missing: Vec<String>,
    pub watched_unwritable: Vec<String>,
}

#[tauri::command]
/// Summary: Tests filesystem access for watched paths and destination.
///
/// Inputs: none.
///
/// Outputs: an `AccessProbe` or an error envelope.
///
/// Side effects: Reads config and filesystem metadata for watched paths.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI diagnostics and support workflows.
///
/// Why this exists: detect permissions and missing paths quickly.
pub fn test_access_cmd() -> Result<AccessProbe, ErrorEnvelope> {
    let cfg = load_validated_config().map_err(|e| {
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

/// Summary: Classifies watched paths by existence and readability.
///
/// Inputs: the loaded config.
///
/// Outputs: three lists: ok, missing, and unwritable paths.
///
/// Side effects: Reads filesystem metadata for watched paths.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: access probing for GUI diagnostics.
///
/// Why this exists: provide precise feedback on watched path issues.
fn classify_watched(cfg: &backup_core::Config) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut watched_ok = Vec::new();
    let mut watched_missing = Vec::new();
    let mut watched_unwritable = Vec::new();
    for w in &cfg.watched {
        if !w.path.exists() {
            watched_missing.push(w.path.display().to_string());
            continue;
        }
        if let Err(e) = run_blocking_io("gui::access::classify_watched metadata", || {
            fs::metadata(&w.path).with_context(|| {
                format!(
                    "access::classify_watched failed reading metadata for {:?}",
                    w.path
                )
            })
        }) {
            watched_unwritable.push(format!("{} (metadata failed: {})", w.path.display(), e));
            continue;
        }
        watched_ok.push(w.path.display().to_string());
    }
    (watched_ok, watched_missing, watched_unwritable)
}

/// Summary: Probes the destination path for writability and free space.
///
/// Inputs: the destination path.
///
/// Outputs: a tuple of (is_writable, message).
///
/// Side effects: Creates destination directories and reads free space metadata.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: access probing for GUI diagnostics.
///
/// Why this exists: surface destination issues with actionable messaging.
fn probe_destination(dest: &std::path::Path) -> (bool, String) {
    let dest_msg = if dest.as_os_str().is_empty() {
        "No backup destination set".to_string()
    } else if dest.exists() && dest.is_file() {
        format!("{:?} is a file; choose a folder", dest)
    } else {
        match run_blocking_io("gui::access::probe_destination create destination", || {
            fs::create_dir_all(dest)
                .with_context(|| format!("access::probe_destination failed to create {:?}", dest))
        }) {
            Ok(_) => match free_space(dest) {
                Ok(free) => format!("Writable. Free space: {} bytes", free),
                Err(e) => format!("Writable, but failed to read free space: {}", e),
            },
            Err(e) => format!("Not writable: {}", e),
        }
    };
    (dest_msg.starts_with("Writable"), dest_msg)
}
