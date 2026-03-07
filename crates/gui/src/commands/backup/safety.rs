use crate::api::{config_api, status_api};
use crate::commands::correlation;
use crate::commands::error::ErrorEnvelope;
use backup_core::{
    backup::versioned, load_validated_config, logging::redact_path, platform::paths,
    state::store::StateStore,
};
use std::path::PathBuf;
use tauri::async_runtime;
use tracing::warn;

/// Summary: Resolve a watched path entry from a user-provided path string.
///
/// Inputs: Loaded config and the user-provided watched path string.
///
/// Outputs: The matched watched path index within `cfg.watched`.
///
/// Side effects: Best-effort canonicalization of paths.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Safety baseline removal flows for large deletion events.
///
/// Why this exists: UI warnings carry paths as strings; matching must be tolerant and deterministic.
fn locate_watched_index(cfg: &backup_core::Config, watched_path: &str, cid: &str) -> Option<usize> {
    let requested = PathBuf::from(watched_path);
    let requested_canon = match requested.canonicalize() {
        Ok(path) => Some(path),
        Err(error) => {
            warn!(
                cid = %cid,
                watched_path = %redact_path(&requested),
                error = %error,
                "safety::locate_watched_index failed to canonicalize requested path"
            );
            None
        }
    };
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
/// Summary: Removes the extra safety baseline version kept after large deletions.
///
/// Inputs: The watched path string and optional correlation id.
///
/// Outputs: `Ok(())` when the baseline was removed or an error envelope.
///
/// Side effects: Updates the versioned store, clears the daemon safety warning over IPC, and persists state best-effort.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Minimal UI safety warning banner and tray warning states.
///
/// Why this exists: When a shrink is expected, the user should be able to drop the extra kept version explicitly.
pub async fn remove_kept_extra_version_cmd(
    watched_path: String,
    correlation_id: Option<String>,
) -> Result<(), ErrorEnvelope> {
    let cid = correlation::cid("safety_remove", correlation_id);
    let cfg = load_validated_config().map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_LOAD",
            format!(
                "[cid={}] safety::remove_kept_extra_version_cmd failed to load config: {}",
                cid, e
            ),
        )
    })?;

    let idx = locate_watched_index(&cfg, &watched_path, &cid).ok_or_else(|| {
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
    let cid_clear = cid.clone();
    async_runtime::spawn(async move {
        if let Err(e) =
            status_api::clear_safety_warning_with_correlation(Some(cid_clear.as_str())).await
        {
            warn!(
                cid = %cid_clear,
                action = "clear_safety_warning_failed",
                error = %e,
                "safety::remove_kept_extra_version_cmd failed to clear daemon warning"
            );
        }
    });

    // Best-effort: clear persisted warning as well so it does not reappear after restarts.
    let cid_state = cid.clone();
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
                    if let Err(error) = store.persist(&state) {
                        warn!(
                            cid = %cid_state,
                            action = "persist_state_failed",
                            error = %error,
                            "safety::remove_kept_extra_version_cmd failed to persist state after clearing safety warning"
                        );
                    }
                }
            }
        }
    });

    // Update config cache if the GUI is running in the same process.
    if let Err(error) = config_api::get_config() {
        warn!(
            cid = %cid,
            action = "config_cache_refresh_failed",
            error = %error,
            "safety::remove_kept_extra_version_cmd failed to refresh config cache"
        );
    }

    Ok(())
}
