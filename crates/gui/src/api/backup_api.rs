use anyhow::{anyhow, Context, Result};
use backup_core::state::store::StateStore;
use std::path::{Path, PathBuf};

/// Purpose: Loads the current stored state from disk.
///
/// Inputs: none.
/// Outputs: the loaded stored state.
/// Ties to: GUI backup browsing operations.
/// Side effects: Reads the state file from disk.
/// Why: provide a single point for loading state before browsing backups.
fn load_state() -> Result<backup_core::state::StoredState> {
    let state_path = backup_core::platform::paths::state_file_path()
        .context("gui::api::backup_api::load_state failed to resolve state path")?;
    let (state, _) = StateStore::load_or_default(state_path)
        .context("gui::api::backup_api::load_state failed to load state")?;
    Ok(state)
}

/// Purpose: Returns the list of tracked source paths from state.
///
/// Inputs: none.
/// Outputs: a list of tracked source path strings.
/// Ties to: GUI backup browsing and selection UI.
/// Side effects: Reads the state file from disk.
/// Why: support UI selection of available backups.
pub fn list_tracked_paths() -> Result<Vec<String>> {
    let state = load_state()?;
    Ok(state.files.keys().cloned().collect())
}

/// Purpose: Returns the backup copies recorded for a specific source path.
///
/// Inputs: the source path.
/// Outputs: a list of backup file paths.
/// Ties to: GUI backup browsing and detail panels.
/// Side effects: Reads the state file from disk.
/// Why: allow users to inspect backup history for a specific file.
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
