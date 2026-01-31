pub mod cycle;
pub mod ipc;
pub mod logging;
pub mod r#loop;
pub mod signals;

use anyhow::Result;
use backup_core::{Config, StateStore, StoredState};

/// Purpose: Starts the daemon runtime loop with the provided config and state.
///
/// Inputs: config, state, and state store.
/// Outputs: `Ok(())` when the runtime loop exits cleanly.
/// Ties to: daemon entry point and runtime orchestration.
/// Side effects: Starts the runtime loop which spawns background tasks.
/// Why: provide a single entry point to the runtime loop.
pub async fn run_daemon(cfg: Config, state: StoredState, store: StateStore) -> Result<()> {
    r#loop::run(cfg, state, store).await
}
