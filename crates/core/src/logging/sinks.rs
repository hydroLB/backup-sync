use crate::io::{run_with_policy, BlockingIoPolicy, CancellationFlag};
use anyhow::{Context, Result};
use std::path::Path;
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};

/// Summary: Creates a non blocking rolling file sink for logs.
///
/// Inputs: the desired log file path.
///
/// Outputs: a writer and worker guard for tracing.
///
/// Side effects: Creates log directories and opens rolling log files.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: logging initialization for file output.
///
/// Why this exists: provide a reusable file sink helper with error context.
pub fn rolling_file_sink(path: &Path) -> Result<(NonBlocking, WorkerGuard)> {
    let (parent, file_name) = match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) => (parent, name.to_string_lossy().to_string()),
        _ => (std::path::Path::new("."), "backup_sync.log".to_string()),
    };
    let io_policy = BlockingIoPolicy::bootstrap_defaults();
    run_with_policy(
        "logging::sinks::rolling_file_sink create log directory",
        &io_policy,
        CancellationFlag::none(),
        || {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "logging::sinks::rolling_file_sink failed to create log directory {:?}",
                    parent
                )
            })
        },
    )?;
    let rolling = tracing_appender::rolling::daily(parent, file_name);
    Ok(tracing_appender::non_blocking(rolling))
}
