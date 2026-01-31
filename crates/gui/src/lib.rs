//! GUI crate surface: exposes API modules for Tauri commands and tray helpers.
pub mod api;
pub mod app;
pub mod commands;
pub mod tray;

/// Purpose: Runs the GUI application wiring shared by all GUI binaries.
///
/// Inputs: none.
/// Outputs: `Ok(())` when the GUI exits cleanly.
/// Side effects: Starts the Tauri runtime and spawns background tasks.
/// Error handling: Returns a boxed error with the underlying failure cause.
/// Ties to other methods: `app::run`.
/// Why this exists: keep the crate's primary entry point stable for downstream binaries.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    app::run()
}
