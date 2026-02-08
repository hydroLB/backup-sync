use fs2::FileExt;
use std::fs::OpenOptions;
use std::path::PathBuf;
use tauri::Manager;

struct SingleInstanceGuard {
    _file: std::fs::File,
}

/// Purpose: Initializes a best-effort single-instance guard for Tauri applications.
///
/// Inputs: A callback invoked in the second instance right before it exits.
/// Outputs: A Tauri plugin to be registered on the app builder.
/// Side effects: Creates and exclusively locks a per-app lock file. Exits the process when the lock
/// cannot be acquired.
/// Error handling: Returns a plugin; lock acquisition failures terminate the process to avoid
/// spawning multiple instances without coordination.
/// Ties to other methods: Intended to be used from `tauri::Builder::plugin` wiring.
/// Why this exists: Avoids a git dependency for single-instance behavior while keeping the GUI
/// buildable in restricted environments.
pub fn init<F>(on_second_instance: F) -> tauri::plugin::TauriPlugin<tauri::Wry>
where
    F: Fn(&tauri::AppHandle, Vec<String>, String) + Send + Sync + 'static,
{
    tauri::plugin::Builder::new("single-instance")
        .setup(move |app| {
            let lock_path = lock_path(app)?;
            let file = OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .open(&lock_path)
                .map_err(|e| {
                    tauri::Error::Io(std::io::Error::new(
                        e.kind(),
                        format!(
                            "tauri_plugin_single_instance::init failed opening lock {:?}: {}",
                            lock_path, e
                        ),
                    ))
                })?;
            if let Err(_e) = file.try_lock_exclusive() {
                let argv = std::env::args().collect::<Vec<_>>();
                let cwd = std::env::current_dir()
                    .ok()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
                on_second_instance(app, argv, cwd);
                std::process::exit(0);
            }
            app.manage(SingleInstanceGuard { _file: file });
            Ok(())
        })
        .build()
}

/// Purpose: Resolves a stable lock-file path for the current application.
///
/// Inputs: The Tauri app handle used for identifier and directory lookup.
/// Outputs: A filesystem path for the single-instance lock file.
/// Side effects: None.
/// Error handling: Falls back to the OS temp dir when app data directories are unavailable.
/// Ties to other methods: Used by `init` during plugin setup.
/// Why this exists: Keep locking per-app and per-user without hardcoding platform paths.
fn lock_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf, tauri::Error> {
    let cfg = app.config();
    let identifier = cfg.tauri.bundle.identifier.clone();
    let base =
        tauri::api::path::app_cache_dir(cfg.as_ref()).unwrap_or_else(|| std::env::temp_dir());
    Ok(base.join(format!("{identifier}.single_instance.lock")))
}
