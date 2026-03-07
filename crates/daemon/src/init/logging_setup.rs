use anyhow::{Context, Result};
use std::path::PathBuf;

/// Summary: Initializes daemon logging with optional file output.
///
/// Inputs: an optional log file path override.
///
/// Outputs: `Ok(())` when logging is configured.
///
/// Side effects: Initializes global logging subscribers and opens log sinks.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon startup logging configuration.
///
/// Why this exists: centralize logging setup with consistent error context.
pub fn init_logging(log_path: Option<PathBuf>) -> Result<()> {
    backup_core::logging::init_with_optional_file(log_path)
        .context("daemon::init::init_logging failed to initialize logging")
}
