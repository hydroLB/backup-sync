mod cycle;
mod ipc;
mod logging;
mod r#loop;
mod signals;

use anyhow::Result;
use backup_core::{Config, StateStore, StoredState};

pub use ipc::{socket_path, spawn_server};
pub use logging::cid;

/// Summary: Starts the daemon runtime loop with the provided config and state.
///
/// Inputs: config, state, and state store.
///
/// Outputs: `Ok(())` when the runtime loop exits cleanly.
///
/// Side effects: Starts the runtime loop which spawns background tasks.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon entry point and runtime orchestration.
///
/// Why this exists: provide a single entry point to the runtime loop.
pub async fn run_daemon(cfg: Config, state: StoredState, store: StateStore) -> Result<()> {
    r#loop::run(cfg, state, store).await
}
