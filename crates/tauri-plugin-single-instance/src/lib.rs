use fs2::FileExt;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tauri::Manager;
use tracing::warn;

const LOCK_OPEN_RETRY_DELAYS_MS: [u64; 3] = [100, 200, 400];
const LOCK_OPEN_TIMEOUT_SECONDS: u64 = 3;

/// Summary: Generate a lightweight correlation id for plugin logs.
///
/// Inputs: a static prefix.
///
/// Outputs: a timestamp-based correlation id.
///
/// Side effects: Reads system time.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: plugin setup warnings and second-instance activation logs.
///
/// Why this exists: keep single-instance plugin logs traceable without extra dependencies.
fn cid(prefix: &str) -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{prefix}-{ts}")
}

struct SingleInstanceGuard {
    _file: std::fs::File,
}

/// Summary: Opens the single-instance lock file with explicit timeout and retry behavior.
///
/// Inputs: lock file path.
///
/// Outputs: opened lock file handle.
///
/// Side effects: Creates the lock file when missing and sleeps between retries.
///
/// Error handling: Returns a contextual tauri IO error if all attempts fail or timeout is reached.
///
/// Ties to other methods: `init` lock acquisition.
///
/// Why this exists: lock-file I/O should have explicit bounded retry behavior like other runtime I/O paths.
fn open_lock_file_with_retry(lock_path: &std::path::Path) -> Result<std::fs::File, tauri::Error> {
    let timeout = Duration::from_secs(LOCK_OPEN_TIMEOUT_SECONDS);
    let start = Instant::now();
    let mut last_error: Option<std::io::Error> = None;

    for (attempt_idx, delay_ms) in LOCK_OPEN_RETRY_DELAYS_MS.into_iter().enumerate() {
        match OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)
        {
            Ok(file) => return Ok(file),
            Err(error) => {
                last_error = Some(error);
                if start.elapsed() > timeout {
                    break;
                }
                if attempt_idx < LOCK_OPEN_RETRY_DELAYS_MS.len() - 1 {
                    std::thread::sleep(Duration::from_millis(delay_ms));
                }
            }
        }
    }

    let lock_name = lock_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("<unknown>");
    let error = last_error.unwrap_or_else(|| {
        std::io::Error::other(
            "tauri_plugin_single_instance::open_lock_file_with_retry unknown lock open error",
        )
    });
    Err(tauri::Error::Io(std::io::Error::new(
        error.kind(),
        format!(
            "tauri_plugin_single_instance::open_lock_file_with_retry failed opening lock file {} after retries: {}",
            lock_name, error
        ),
    )))
}

/// Summary: Initializes a best-effort single-instance guard for Tauri applications.
///
/// Inputs: A callback invoked in the second instance right before it exits.
///
/// Outputs: A Tauri plugin to be registered on the app builder.
///
/// Side effects: Creates and exclusively locks a per-app lock file. Exits the process when the lock
///
/// Error handling: Returns a plugin; lock acquisition failures terminate the process to avoid
///
/// Ties to other methods: Intended to be used from `tauri::Builder::plugin` wiring.
///
/// Why this exists: Avoids a git dependency for single-instance behavior while keeping the GUI
pub fn init<F>(on_second_instance: F) -> tauri::plugin::TauriPlugin<tauri::Wry>
where
    F: Fn(&tauri::AppHandle, Vec<String>, String) + Send + Sync + 'static,
{
    tauri::plugin::Builder::new("single-instance")
        .setup(move |app| {
            let lock_path = lock_path(app)?;
            let file = open_lock_file_with_retry(&lock_path)?;
            if let Err(error) = file.try_lock_exclusive() {
                let lock_cid = cid("single-instance");
                warn!(
                    cid = %lock_cid,
                    action = "lock_unavailable",
                    error = %error,
                    "tauri_plugin_single_instance::init lock already held or unavailable"
                );
                let argv = std::env::args().collect::<Vec<_>>();
                let cwd = match std::env::current_dir() {
                    Ok(path) => path.to_string_lossy().into_owned(),
                    Err(cwd_error) => {
                        warn!(
                            cid = %lock_cid,
                            action = "cwd_resolve_failed",
                            error = %cwd_error,
                            "tauri_plugin_single_instance::init failed to resolve cwd for second-instance callback"
                        );
                        String::new()
                    }
                };
                on_second_instance(app, argv, cwd);
                std::process::exit(0);
            }
            app.manage(SingleInstanceGuard { _file: file });
            Ok(())
        })
        .build()
}

/// Summary: Resolves a stable lock-file path for the current application.
///
/// Inputs: The Tauri app handle used for identifier and directory lookup.
///
/// Outputs: A filesystem path for the single-instance lock file.
///
/// Side effects: None.
///
/// Error handling: Falls back to the OS temp dir when app data directories are unavailable.
///
/// Ties to other methods: Used by `init` during plugin setup.
///
/// Why this exists: Keep locking per-app and per-user without hardcoding platform paths.
fn lock_path<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf, tauri::Error> {
    let cfg = app.config();
    let identifier = cfg.tauri.bundle.identifier.clone();
    let base = tauri::api::path::app_cache_dir(cfg.as_ref()).unwrap_or_else(std::env::temp_dir);
    Ok(base.join(format!("{identifier}.single_instance.lock")))
}
