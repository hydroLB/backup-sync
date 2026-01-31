//! GUI entrypoint: delegates to the shared GUI launcher.
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

/// Purpose: Starts the GUI process and exits on startup failure.
///
/// Inputs: none.
/// Outputs: `()` after the GUI exits or the process terminates.
/// Ties to: `gui::run`.
/// Side effects: Launches the Tauri runtime and may terminate the process on error.
/// Why: provide a single entry point with explicit failure handling.
fn main() {
    if let Err(err) = gui::run() {
        eprintln!("failed to run tauri app: {err}");
        std::process::exit(1);
    }
}
