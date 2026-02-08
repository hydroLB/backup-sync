use crate::api::{config_api, status_api};
use crate::commands::correlation;
use crate::commands::error::ErrorEnvelope;
use backup_core::{backup::versioned, load_config, platform::paths, state::store::StateStore, validate};
use std::path::PathBuf;
use tauri::async_runtime;
use tracing::warn;

/// Purpose: Resolve a watched path entry from a user-provided path string.
///
/// Inputs: Loaded config and the user-provided watched path string.
/// Outputs: The matched watched path index within `cfg.watched`.
/// Ties to: Safety baseline removal flows for large deletion events.
/// Side effects: Best-effort canonicalization of paths.
/// Why: UI warnings carry paths as strings; matching must be tolerant and deterministic.
fn locate_watched_index(cfg: &backup_core::Config, watched_path: &str) -> Option<usize> {
    let requested = PathBuf::from(watched_path);
    let requested_canon = requested.canonicalize().ok();
    cfg.watched.iter().position(|w| {
        if w.path == requested {
            return true;
        }
        if let (Some(a), Ok(b)) = (requested_canon.as_ref(), w.path.canonicalize()) {
            if &b == a {
                return true;
            }
        }
        w.path.to_string_lossy() == watched_path
    })
}

#[tauri::command]
/// Purpose: Removes the extra safety baseline version kept after large deletions.
///
/// Inputs: The watched path string and optional correlation id.
/// Outputs: `Ok(())` when the baseline was removed or an error envelope.
/// Ties to: Minimal UI safety warning banner and tray warning states.
/// Side effects: Updates the versioned store, clears the daemon safety warning over IPC, and persists state best-effort.
/// Why: When a shrink is expected, the user should be able to drop the extra kept version explicitly.
pub async fn remove_kept_extra_version_cmd(
    watched_path: String,
    correlation_id: Option<String>,
) -> Result<(), ErrorEnvelope> {
    let cid = correlation::cid("safety_remove", correlation_id);
    let cfg = load_config().map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_LOAD",
            format!(
                "[cid={}] safety::remove_kept_extra_version_cmd failed to load config: {}",
                cid, e
            ),
        )
    })?;
    validate(&cfg).map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_INVALID",
            format!(
                "[cid={}] safety::remove_kept_extra_version_cmd config validation failed: {}",
                cid, e
            ),
        )
    })?;

    let idx = locate_watched_index(&cfg, &watched_path).ok_or_else(|| {
        ErrorEnvelope::new(
            "WATCHED_NOT_FOUND",
            format!(
                "[cid={}] safety::remove_kept_extra_version_cmd watched path not found: {}",
                cid, watched_path
            ),
        )
    })?;
    let watched = &cfg.watched[idx];

    let dest = cfg
        .destinations
        .iter()
        .find(|d| d.id == watched.destination_id)
        .ok_or_else(|| {
            ErrorEnvelope::new(
                "DEST_NOT_FOUND",
                format!(
                    "[cid={}] safety::remove_kept_extra_version_cmd destination id {} missing for watched path {}",
                    cid, watched.destination_id, watched_path
                ),
            )
        })?;

    let removed = versioned::remove_kept_safety_version(&dest.path, &watched.path).map_err(|e| {
        ErrorEnvelope::new(
            "SAFETY_REMOVE_FAILED",
            format!(
                "[cid={}] safety::remove_kept_extra_version_cmd failed to remove safety baseline: {}",
                cid, e
            ),
        )
    })?;
    if removed.is_none() {
        return Err(ErrorEnvelope::new(
            "SAFETY_NOT_PRESENT",
            format!(
                "[cid={}] No extra kept version exists for {}",
                cid, watched_path
            ),
        ));
    }

    // Clear the warning in the daemon so tray + UI stop surfacing it.
    async_runtime::spawn(async move {
        if let Err(e) = status_api::clear_safety_warning().await {
            warn!("safety::remove_kept_extra_version_cmd failed to clear daemon warning: {e}");
        }
    });

    // Best-effort: clear persisted warning as well so it does not reappear after restarts.
    let watched_path_for_state = watched_path.clone();
    async_runtime::spawn(async move {
        if let Ok(state_path) = paths::state_file_path() {
            if let Ok((mut state, store)) = StateStore::load_or_default(state_path) {
                if state
                    .last_safety_warning
                    .as_ref()
                    .and_then(|w| w.watched_path.as_deref())
                    == Some(watched_path_for_state.as_str())
                {
                    state.last_safety_warning = None;
                    let _ = store.persist(&state);
                }
            }
        }
    });

    // Update config cache if the GUI is running in the same process.
    let _ = config_api::get_config();

    Ok(())
}
