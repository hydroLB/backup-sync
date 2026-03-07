use tracing::error;

/// Summary: Starts the Tauri GUI runtime and handles startup failures.
///
/// Inputs: none.
///
/// Outputs: `()` after launching the GUI or exiting on failure.
///
/// Side effects: Starts the GUI process and may terminate the process on error.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: desktop app entrypoint handling.
///
/// Why this exists: provide a thin entrypoint with explicit failure handling.
fn main() {
    if let Err(err) = gui::run() {
        let cid = gui::observability::cid("gui-main", None);
        let error_text = backup_core::logging::redact_text(&err.to_string());
        error!(
            cid = %cid,
            action = "startup_failed",
            error = %error_text,
            "failed to start gui"
        );
        std::process::exit(1);
    }
}
