use anyhow::Result;
use backup_core::io::{run_with_policy, BlockingIoPolicy, CancellationFlag};
use backup_core::load_validated_config;
use tracing::warn;

/// Keep timeout and retry behavior consistent across GUI command I/O boundaries.
pub fn resolve_io_policy() -> BlockingIoPolicy {
    match load_validated_config() {
        Ok(cfg) => BlockingIoPolicy::from_config(&cfg),
        Err(error) => {
            warn!(
                "gui::commands::io_policy::resolve_io_policy failed to load config; using defaults: {}",
                error
            );
            BlockingIoPolicy::bootstrap_defaults()
        }
    }
}

/// Avoid ad-hoc timeout and retry behavior across GUI command modules.
pub fn run_blocking_io<T, F>(label: &str, op: F) -> Result<T>
where
    F: FnMut() -> Result<T>,
{
    let policy = resolve_io_policy();
    run_with_policy(label, &policy, CancellationFlag::none(), op)
}
