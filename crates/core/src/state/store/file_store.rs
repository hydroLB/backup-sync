use crate::state::models::StoredState;
use crate::{io::run_with_policy, io::BlockingIoPolicy, io::CancellationFlag};
use anyhow::{Context, Result};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

/// Keep state durable across runs.
#[derive(Clone)]
pub struct StateStore {
    path: PathBuf,
}

impl StateStore {
    /// Centralize state storage configuration in one struct.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Allow first run initialization without failing.
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

    /// Keep state durable and recoverable across runs.
    pub fn persist(&self, state: &StoredState) -> Result<()> {
        let io_policy = BlockingIoPolicy::bootstrap_defaults();
        let raw = serde_json::to_string_pretty(state)
            .context("state::store::persist failed to serialize state to JSON")?;
        let parent = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));

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

        run_with_policy(
            "state::store::persist replace state file",
            &io_policy,
            CancellationFlag::none(),
            || {
                let mut temp = tempfile::NamedTempFile::new_in(parent)
                    .context("state::store::persist failed to create temporary state file")?;
                temp.write_all(raw.as_bytes())
                    .context("state::store::persist failed to write temporary state file")?;
                temp.flush()
                    .context("state::store::persist failed to flush temporary state file")?;
                temp.as_file()
                    .sync_all()
                    .context("state::store::persist failed to sync temporary state file")?;
                temp.persist(&self.path)
                    .map_err(|error| error.error)
                    .with_context(|| {
                        format!(
                            "state::store::persist failed to replace state file {:?}",
                            self.path
                        )
                    })?;
                sync_parent(parent)
            },
        )?;
        Ok(())
    }
}

#[cfg(target_family = "unix")]
fn sync_parent(parent: &Path) -> Result<()> {
    let directory = fs::File::open(parent)
        .context("state::store::persist failed to open state directory for sync")?;
    directory
        .sync_all()
        .context("state::store::persist failed to sync state directory")
}

#[cfg(not(target_family = "unix"))]
fn sync_parent(_parent: &Path) -> Result<()> {
    Ok(())
}
