mod cycle;
mod ipc;
mod logging;
mod r#loop;
mod signals;

use anyhow::Result;
use backup_core::{Config, StateStore, StoredState};

pub use ipc::{socket_path, spawn_server};
pub use logging::cid;

/// Provide a single entry point to the runtime loop.
pub async fn run_daemon(cfg: Config, state: StoredState, store: StateStore) -> Result<()> {
    r#loop::run(cfg, state, store).await
}
