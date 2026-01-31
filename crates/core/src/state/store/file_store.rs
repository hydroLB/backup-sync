use crate::state::models::StoredState;
use anyhow::{Context, Result};
use std::{fs, path::PathBuf};

/// Purpose: Persists and loads backup state to a JSON file.
///
/// Inputs: a filesystem path to the state file.
/// Outputs: a load or save operation for `StoredState`.
/// Ties to: daemon, CLI, and GUI state management.
/// Side effects: None.
/// Why: keep state durable across runs.
#[derive(Clone)]
pub struct StateStore {
    path: PathBuf,
}

impl StateStore {
    /// Purpose: Builds a new state store for the given path.
    ///
    /// Inputs: the desired state file path.
    /// Outputs: a `StateStore` configured with the path.
    /// Ties to: state persistence setup.
    /// Side effects: None.
    /// Why: centralize state storage configuration in one struct.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Purpose: Loads state from disk or returns a default state when missing.
    ///
    /// Inputs: the state file path.
    /// Outputs: the stored or default state plus a `StateStore` for persistence.
    /// Ties to: application startup flows.
    /// Side effects: Reads the state file from disk when present.
    /// Why: allow first run initialization without failing.
    pub fn load_or_default(path: PathBuf) -> Result<(StoredState, StateStore)> {
        if path.exists() {
            let raw = fs::read_to_string(&path).with_context(|| {
                format!(
                    "state::store::load_or_default failed to read state file {:?}",
                    path
                )
            })?;
            let state: StoredState = serde_json::from_str(&raw).with_context(|| {
                format!(
                    "state::store::load_or_default failed to parse state JSON at {:?}",
                    path
                )
            })?;
            Ok((state, StateStore { path }))
        } else {
            Ok((StoredState::default(), StateStore { path }))
        }
    }

    /// Purpose: Persists the current state to disk as JSON.
    ///
    /// Inputs: the state to serialize and persist.
    /// Outputs: `Ok(())` when the file is written.
    /// Ties to: backup execution and verification flows.
    /// Side effects: Creates directories and writes the state file to disk.
    /// Why: keep state durable and recoverable across runs.
    pub fn persist(&self, state: &StoredState) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "state::store::persist failed to create state dir {:?}",
                    parent
                )
            })?;
        }
        let raw = serde_json::to_string_pretty(state)
            .context("state::store::persist failed to serialize state to JSON")?;
        fs::write(&self.path, raw).with_context(|| {
            format!(
                "state::store::persist failed to write state file {:?}",
                self.path
            )
        })?;
        Ok(())
    }
}
