use tracing::error;

/// Provide a thin entrypoint with explicit failure handling.
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
