use anyhow::Result;
use backup_core::io::{run_with_policy, BlockingIoPolicy, CancellationFlag};
use backup_core::load_validated_config;
use tracing::warn;

/// Summary: Resolves blocking I/O policy for GUI command handlers.
///
/// Inputs: none.
///
/// Outputs: Policy from config when available, otherwise bootstrap defaults.
///
/// Side effects: Reads config file and logs fallback warnings.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `run_blocking_io` and file-bound GUI command handlers.
///
/// Why this exists: Keep timeout and retry behavior consistent across GUI command I/O boundaries.
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

/// Summary: Executes blocking I/O under the shared GUI policy.
///
/// Inputs: operation label and fallible closure.
///
/// Outputs: Closure result when successful.
///
/// Side effects: Applies retry backoff sleeps and cancellation checks.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI command handlers that perform filesystem I/O.
///
/// Why this exists: Avoid ad-hoc timeout and retry behavior across GUI command modules.
pub fn run_blocking_io<T, F>(label: &str, op: F) -> Result<T>
where
    F: FnMut() -> Result<T>,
{
    let policy = resolve_io_policy();
    run_with_policy(label, &policy, CancellationFlag::none(), op)
}
