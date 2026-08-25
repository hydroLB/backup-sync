//! GUI crate public API surface.
//!
//! External callers should use:
//! - [`run`] to launch the GUI runtime.
//! - [`observability::cid`] for correlation ids in startup/error logs.
mod api;
mod app;
mod commands;
mod tray;
use backup_core::logging::redact_path;
use tracing::{info, warn};

/// Expose a stable, explicit observability API without exporting command internals.
pub mod observability {
    pub use crate::commands::correlation::cid;
}

/// Ensure GUI observability uses structured logs with consistent sinks.
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

/// Keep the crate's primary entry point stable for downstream binaries.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    init_gui_logging();
    // Start the shell even when an existing config is malformed. The frontend can
    // then show a retryable configuration error instead of making the entire app
    // disappear before the user has any recovery guidance.
    app::run()
}
