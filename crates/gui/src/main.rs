//! GUI entrypoint: delegates to the shared GUI launcher.
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use tracing::error;

/// Provide a single entry point with explicit failure handling.
fn main() {
    if let Err(err) = gui::run() {
        let classified = backup_core::classify_anyhow(&anyhow::Error::msg(err.to_string()));
        let cid = gui::observability::cid("gui-main", None);
        let error_text = backup_core::logging::redact_text(&err.to_string());
        error!(
            cid = %cid,
            action = "startup_failed",
            code = %classified.code.as_str(),
            retryable = classified.retryable,
            hint = %classified.hint,
            error = %error_text,
            "failed to run tauri app"
        );
        eprintln!(
            "{}: {}. Hint: {}",
            classified.code.as_str(),
            error_text,
            classified.hint
        );
        std::process::exit(1);
    }
}
