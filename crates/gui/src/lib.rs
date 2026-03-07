//! GUI crate public API surface.
//!
//! External callers should use:
//! - [`run`] to launch the GUI runtime.
//! - [`observability::cid`] for correlation ids in startup/error logs.
use anyhow::Context;
mod api;
mod app;
mod commands;
mod tray;
use backup_core::logging::redact_path;
use tracing::{info, warn};

/// Summary: Public observability helpers for callers that need GUI correlation ids.
///
/// Inputs: delegated to underlying observability helpers.
///
/// Outputs: delegated to underlying observability helpers.
///
/// Side effects: delegated to underlying observability helpers.
///
/// Error handling: delegated to underlying observability helpers.
///
/// Ties to other methods: `commands::correlation`.
///
/// Why this exists: expose a stable, explicit observability API without exporting command internals.
pub mod observability {
    pub use crate::commands::correlation::cid;
}

/// Summary: Initialize structured logging for the GUI process.
///
/// Inputs: none.
///
/// Outputs: `()` after best-effort logger initialization.
///
/// Side effects: Initializes the global tracing subscriber and opens log sinks.
///
/// Error handling: Falls back to stdout logging when file logging setup fails.
///
/// Ties to other methods: `run` entrypoint and all GUI tracing events.
///
/// Why this exists: ensure GUI observability uses structured logs with consistent sinks.
fn init_gui_logging() {
    let cid = commands::correlation::cid("gui-log-init", None);
    let log_path = match backup_core::platform::paths::log_file_path() {
        Ok(path) => Some(path),
        Err(error) => {
            warn!(
                cid = %cid,
                error = %error,
                "gui log path unavailable; continuing with stdout sink fallback"
            );
            None
        }
    };
    if let Err(error) = backup_core::logging::init_with_optional_file(log_path.clone()) {
        backup_core::logging::init();
        warn!(
            cid = %cid,
            error = %error,
            "gui logging file sink init failed; fell back to stdout logging"
        );
        return;
    }

    if let Some(path) = log_path.as_ref() {
        info!(
            cid = %cid,
            log_path = %redact_path(path),
            "gui logging initialized"
        );
    } else {
        info!(cid = %cid, "gui logging initialized without file sink");
    }
}

/// Summary: Runs the GUI application wiring shared by all GUI binaries.
///
/// Inputs: none.
///
/// Outputs: `Ok(())` when the GUI exits cleanly.
///
/// Side effects: Starts the Tauri runtime and spawns background tasks.
///
/// Error handling: Returns a boxed error with the underlying failure cause.
///
/// Ties to other methods: `app::run`.
///
/// Why this exists: keep the crate's primary entry point stable for downstream binaries.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    init_gui_logging();
    backup_core::load_validated_config().context("gui::run startup config preflight failed")?;
    app::run()
}
