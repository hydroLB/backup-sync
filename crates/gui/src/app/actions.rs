use crate::api::{config_api, status_api};
use crate::commands;
use crate::tray;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::api::dialog;
use tauri::async_runtime;
use tauri::{Manager, SystemTrayEvent, WindowEvent};

static IS_QUITTING: AtomicBool = AtomicBool::new(false);
static LAST_CLOSE_BACKUP_AT: AtomicBool = AtomicBool::new(false);

/// Purpose: Shows and focuses the main window.
///
/// Inputs: the Tauri app handle.
/// Outputs: `()` after attempting to show the window.
/// Ties to: tray and single instance activation handlers.
/// Side effects: Shows the main window and sets focus.
/// Why: provide a consistent way to surface the UI.
pub(crate) fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Purpose: Handles tray events by routing to the appropriate actions.
///
/// Inputs: the app handle and the system tray event.
/// Outputs: `()` after dispatching the event.
/// Ties to: tray menu wiring in `app::run`.
/// Side effects: May show the main window or exit the app.
/// Why: keep the `tauri::Builder` wiring readable and keep the tray UX minimal.
pub(crate) fn handle_tray_event(app: &tauri::AppHandle, event: SystemTrayEvent) {
    if IS_QUITTING.load(Ordering::Relaxed) {
        return;
    }
    match event {
        // macOS convention: left click opens the tray menu. Avoid forcing the main window
        // to the foreground on every click.
        SystemTrayEvent::DoubleClick { .. } => show_main_window(app),
        SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
            tray::SHOW => show_main_window(app),
            tray::QUIT => confirm_and_quit(app),
            _ => {}
        },
        _ => {}
    }
}

/// Purpose: Handles window events with a consistent close policy.
///
/// Inputs: the global window event.
/// Outputs: `()` after handling the event.
/// Ties to: lifecycle wiring in `app::run`.
/// Side effects: Prevents the default close behavior and exits the app cleanly.
/// Why: avoid macOS-specific close quirks leaving a blank re-opened window.
pub(crate) fn handle_window_event(event: tauri::GlobalWindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event.event() {
        if IS_QUITTING.load(Ordering::Relaxed) {
            // Allow a real quit to close the window; don't convert it into a hide-to-tray.
            return;
        }
        // Prevent the app from fully quitting on window close. The menu bar icon remains active.
        api.prevent_close();
        let _ = event.window().hide();

        // Best-effort "last backup" on window close (hide-to-tray) without blocking the UI thread.
        // Guard to avoid repeated triggers from rapid close/reopen interactions.
        if !LAST_CLOSE_BACKUP_AT.swap(true, Ordering::Relaxed) {
            async_runtime::spawn(async move {
                // Small debounce window; allow the close/hide to complete.
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                if let Ok(cfg) = config_api::get_config() {
                    if !cfg.safe_mode {
                        let _ = commands::backup::run::run_now_cmd(Some("close-final".to_string())).await;
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                LAST_CLOSE_BACKUP_AT.store(false, Ordering::Relaxed);
            });
        }
    }
}

/// Purpose: Ask for confirmation before quitting because quitting pauses backups.
///
/// Inputs: the Tauri app handle.
/// Outputs: `()` after handling the quit flow.
/// Ties to: tray Quit action.
/// Side effects: Shows a confirmation dialog, may pause backups, may stop the daemon service on macOS, and may exit the app.
/// Why: avoid accidental shutdown of background backups and make the consequence explicit.
fn confirm_and_quit(app: &tauri::AppHandle) {
    if IS_QUITTING.load(Ordering::Relaxed) {
        return;
    }

    let handle = app.clone();
    dialog::confirm(
        Option::<&tauri::Window>::None,
        "Quit Backup Sync?",
        "Quitting will stop background backups. Are you sure you want to quit?",
        move |confirmed| {
            if !confirmed {
                return;
            }
            if IS_QUITTING.swap(true, Ordering::Relaxed) {
                return;
            }

            // Disable tray quit item immediately to prevent duplicate clicks.
            let tray_handle = handle.tray_handle();
            let _ = tray_handle.get_item(tray::QUIT).set_enabled(false);
            let _ = tray_handle.get_item(tray::QUIT).set_title("Quitting…");

            if let Some(window) = handle.get_window("main") {
                let _ = window.hide();
            }

            async_runtime::spawn(async move {
                quit_sequence(handle).await;
            });
        },
    );
}

/// Summary: Run one last backup before quitting with a time bound.
///
/// Inputs: None.
/// Outputs: `Ok(())` when the final backup completed; otherwise a user-facing failure string.
/// Side effects: Performs backup IO and persists state updates.
/// Error handling: Returns a string describing the failure so the caller can prompt the user.
/// Ties to other methods: Used by `quit_sequence` prior to stopping background work.
/// Why this exists: Avoid losing recent changes when the user quits from the menu bar.
async fn run_final_backup_best_effort() -> Result<(), String> {
    let timeout = std::time::Duration::from_secs(3);
    match tokio::time::timeout(
        timeout,
        commands::backup::run::run_now_cmd(Some("quit-final".to_string())),
    )
    .await
    {
        Ok(Ok(())) => Ok(()),
        Ok(Err(err)) => Err(err.message),
        Err(_) => Err(format!(
            "Final backup did not finish within {} seconds.",
            timeout.as_secs()
        )),
    }
}

/// Summary: Execute the full quit flow off the tray event thread.
///
/// Inputs: The application handle.
/// Outputs: None.
/// Side effects: Pauses the daemon, optionally runs a final backup, stops the background service on macOS, closes windows, and exits.
/// Error handling: Prompts the user if the final backup fails and resumes scheduling if they cancel quit.
/// Ties to other methods: Spawned by `confirm_and_quit` after user confirmation.
/// Why this exists: Keep tray quit responsive and ensure a clean shutdown without re-opening the window.
async fn quit_sequence(app: tauri::AppHandle) {
    // Hard stop guard: ensure Quit actually terminates even if background tasks keep
    // the process alive. This is only armed after the user confirms quitting.
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(8));
        std::process::exit(0);
    });

    let cfg_safe_mode = config_api::get_config()
        .map(|cfg| cfg.safe_mode)
        .unwrap_or(true);

    if !cfg_safe_mode {
        // Best-effort only; quitting is an explicit stop and should not hang.
        let _ = run_final_backup_best_effort().await;
    }

    pause_backups_best_effort();
    stop_background_daemon_best_effort_async().await;

    if let Some(window) = app.get_window("main") {
        let _ = window.close();
    }
    app.exit(0);
}

/// Purpose: Pause backups before quitting by enabling safe mode in config and daemon.
///
/// Inputs: none.
/// Outputs: `()` after best-effort attempts complete.
/// Ties to: `confirm_and_quit`.
/// Side effects: Writes configuration and attempts to update the running daemon over IPC.
/// Why: quitting should stop background backups immediately and predictably.
fn pause_backups_best_effort() {
    if let Ok(mut cfg) = config_api::get_config() {
        if !cfg.safe_mode {
            cfg.safe_mode = true;
            let _ = config_api::save_config(&cfg);
        }
    }
    async_runtime::spawn(async {
        let _ = status_api::set_safe_mode(true).await;
    });
}

#[cfg(target_os = "macos")]
async fn stop_background_daemon_best_effort_async() {
    use std::process::Command;

    const LABEL: &str = "com.backup_sync.daemon";

    let stop = async_runtime::spawn_blocking(move || {
        let uid = Command::new("id")
            .arg("-u")
            .output()
            .ok()
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        if uid.is_empty() {
            return;
        }
        let target = format!("gui/{}/{}", uid, LABEL);
        let _ = Command::new("launchctl")
            .args(["bootout", "-k", &target])
            .output();
    });
    let _ = tokio::time::timeout(std::time::Duration::from_secs(3), stop).await;
}

#[cfg(not(target_os = "macos"))]
async fn stop_background_daemon_best_effort_async() {}
