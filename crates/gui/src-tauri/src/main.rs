/// Purpose: Starts the Tauri GUI runtime and handles startup failures.
///
/// Inputs: none.
/// Outputs: `()` after launching the GUI or exiting on failure.
/// Ties to: desktop app entrypoint handling.
/// Side effects: Starts the GUI process and may terminate the process on error.
/// Why: provide a thin entrypoint with explicit failure handling.
fn main() {
    if let Err(err) = gui::run() {
        eprintln!("failed to start gui: {err}");
        std::process::exit(1);
    }
}
