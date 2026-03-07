/// Summary: Runs the Tauri build script for the GUI crate.
///
/// Inputs: none.
///
/// Outputs: `()` after invoking the Tauri build helper.
///
/// Side effects: Generates Tauri build artifacts for the GUI bundle.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: cargo build scripts for the desktop app.
///
/// Why this exists: ensure GUI assets and metadata are prepared during builds.
fn main() {
    tauri_build::build();
}
