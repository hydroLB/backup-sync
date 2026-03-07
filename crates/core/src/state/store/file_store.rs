use crate::state::models::StoredState;
use crate::{io::run_with_policy, io::BlockingIoPolicy, io::CancellationFlag};
use anyhow::{Context, Result};
use std::{fs, path::PathBuf};

/// Summary: Persists and loads backup state to a JSON file.
///
/// Inputs: a filesystem path to the state file.
///
/// Outputs: a load or save operation for `StoredState`.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon, CLI, and GUI state management.
///
/// Why this exists: keep state durable across runs.
#[derive(Clone)]
pub struct StateStore {
    path: PathBuf,
}

impl StateStore {
    /// Summary: Builds a new state store for the given path.
    ///
    /// Inputs: the desired state file path.
    ///
    /// Outputs: a `StateStore` configured with the path.
    ///
    /// Side effects: None.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: state persistence setup.
    ///
    /// Why this exists: centralize state storage configuration in one struct.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Summary: Loads state from disk or returns a default state when missing.
    ///
    /// Inputs: the state file path.
    ///
    /// Outputs: the stored or default state plus a `StateStore` for persistence.
    ///
    /// Side effects: Reads the state file from disk when present.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: application startup flows.
    ///
    /// Why this exists: allow first run initialization without failing.
    pub fn load_or_default(path: PathBuf) -> Result<(StoredState, StateStore)> {
        let io_policy = BlockingIoPolicy::bootstrap_defaults();
        if path.exists() {
            let raw = run_with_policy(
                "state::store::load_or_default read state file",
                &io_policy,
                CancellationFlag::none(),
                || {
                    fs::read_to_string(&path).with_context(|| {
                        format!(
                            "state::store::load_or_default failed to read state file {:?}",
                            path
                        )
                    })
                },
            )?;
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

    /// Summary: Persists the current state to disk as JSON.
    ///
    /// Inputs: the state to serialize and persist.
    ///
    /// Outputs: `Ok(())` when the file is written.
    ///
    /// Side effects: Creates directories and writes the state file to disk.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: backup execution and verification flows.
    ///
    /// Why this exists: keep state durable and recoverable across runs.
    pub fn persist(&self, state: &StoredState) -> Result<()> {
        let io_policy = BlockingIoPolicy::bootstrap_defaults();
        if let Some(parent) = self.path.parent() {
            run_with_policy(
                "state::store::persist create parent directory",
                &io_policy,
                CancellationFlag::none(),
                || {
                    fs::create_dir_all(parent).with_context(|| {
                        format!(
                            "state::store::persist failed to create state dir {:?}",
                            parent
                        )
                    })
                },
            )?;
        }
        let raw = serde_json::to_string_pretty(state)
            .context("state::store::persist failed to serialize state to JSON")?;
        run_with_policy(
            "state::store::persist write state file",
            &io_policy,
            CancellationFlag::none(),
            || {
                fs::write(&self.path, raw.as_bytes()).with_context(|| {
                    format!(
                        "state::store::persist failed to write state file {:?}",
                        self.path
                    )
                })
            },
        )?;
        Ok(())
    }
}
