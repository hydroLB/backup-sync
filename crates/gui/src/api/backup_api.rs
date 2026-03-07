use anyhow::{anyhow, Context, Result};
use backup_core::state::store::StateStore;
use std::path::{Path, PathBuf};

/// Summary: Loads the current stored state from disk.
///
/// Inputs: none.
///
/// Outputs: the loaded stored state.
///
/// Side effects: Reads the state file from disk.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI backup browsing operations.
///
/// Why this exists: provide a single point for loading state before browsing backups.
fn load_state() -> Result<backup_core::state::StoredState> {
    let state_path = backup_core::platform::paths::state_file_path()
        .context("gui::api::backup_api::load_state failed to resolve state path")?;
    let (state, _) = StateStore::load_or_default(state_path)
        .context("gui::api::backup_api::load_state failed to load state")?;
    Ok(state)
}

/// Summary: Returns the list of tracked source paths from state.
///
/// Inputs: none.
///
/// Outputs: a list of tracked source path strings.
///
/// Side effects: Reads the state file from disk.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI backup browsing and selection UI.
///
/// Why this exists: support UI selection of available backups.
pub fn list_tracked_paths() -> Result<Vec<String>> {
    let state = load_state()?;
    Ok(state.files.keys().cloned().collect())
}

/// Summary: Returns the backup copies recorded for a specific source path.
///
/// Inputs: the source path.
///
/// Outputs: a list of backup file paths.
///
/// Side effects: Reads the state file from disk.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI backup browsing and detail panels.
///
/// Why this exists: allow users to inspect backup history for a specific file.
pub fn list_backups_for_path(path: &Path) -> Result<Vec<PathBuf>> {
    let state = load_state()?;
    let key = path.to_string_lossy();
    match state.files.get(key.as_ref()) {
        Some(entry) => Ok(entry.backups.clone()),
        None => Err(anyhow!(
            "gui::api::backup_api::list_backups_for_path no backups tracked for {}",
            key
        )),
    }
}
