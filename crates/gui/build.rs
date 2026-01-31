/// Purpose: Runs the Tauri build script for the GUI crate.
///
/// Inputs: none.
/// Outputs: `()` after invoking the Tauri build helper.
/// Ties to: cargo build scripts for the desktop app.
/// Side effects: Generates Tauri build artifacts for the GUI bundle.
/// Why: ensure GUI assets and metadata are prepared during builds.
fn main() {
    tauri_build::build();
}
