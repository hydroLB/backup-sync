use anyhow::{Context, Result};
use std::path::PathBuf;

/// Centralize logging setup with consistent error context.
pub fn init_logging(log_path: Option<PathBuf>) -> Result<()> {
    backup_core::logging::init_with_optional_file(log_path)
        .context("daemon::init::init_logging failed to initialize logging")
}
