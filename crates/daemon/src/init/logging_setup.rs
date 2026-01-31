use anyhow::{Context, Result};
use std::path::PathBuf;

/// Purpose: Initializes daemon logging with optional file output.
///
/// Inputs: an optional log file path override.
/// Outputs: `Ok(())` when logging is configured.
/// Ties to: daemon startup logging configuration.
/// Side effects: Initializes global logging subscribers and opens log sinks.
/// Why: centralize logging setup with consistent error context.
pub fn init_logging(log_path: Option<PathBuf>) -> Result<()> {
    backup_core::logging::init_with_optional_file(log_path)
        .context("daemon::init::init_logging failed to initialize logging")
}
