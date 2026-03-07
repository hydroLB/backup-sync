use crate::api::{config_api, status_api};
use crate::commands;
use crate::commands::service::common::run_command;
use crate::tray;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::async_runtime;
use tauri::menu::MenuEvent;
use tauri::tray::TrayIconEvent;
use tauri::{Manager, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tracing::warn;

/// Summary: Stores mutable app-action lifecycle flags used by tray/window handlers.
///
/// Inputs: none.
///
/// Outputs: atomics tracking quit state and close-trigger backup debounce state.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `handle_tray_event`, `handle_window_event`, `confirm_and_quit`.
///
/// Why this exists: replace module-level global statics with an injected runtime state object.
#[derive(Default)]
pub(crate) struct ActionState {
    is_quitting: AtomicBool,
    close_backup_in_flight: AtomicBool,
}

pub(crate) type SharedActionState = Arc<ActionState>;

/// Summary: Builds shared action state for tray/window lifecycle handlers.
///
/// Inputs: none.
///
/// Outputs: reference-counted action state.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: injected into tray and window handler closures in `app::run`.
///
/// Why this exists: make lifecycle state explicit and dependency-injected.
pub(crate) fn new_action_state() -> SharedActionState {
    Arc::new(ActionState::default())
}

/// Summary: Shows and focuses the main window.
///
/// Inputs: the Tauri app handle.
///
/// Outputs: `()` after attempting to show the window.
///
/// Side effects: Shows the main window and sets focus.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: tray and single instance activation handlers.
///
/// Why this exists: provide a consistent way to surface the UI.
pub(crate) fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if let Err(error) = window.unminimize() {
            warn!(
                error = %error,
                "app::actions::show_main_window failed to unminimize window"
            );
        }
        if let Err(error) = window.show() {
            warn!(
                error = %error,
                "app::actions::show_main_window failed to show window"
            );
        }
        if let Err(error) = window.set_focus() {
            warn!(
                error = %error,
                "app::actions::show_main_window failed to focus window"
            );
        }
    }
}

/// Summary: Handles tray events by routing to the appropriate actions.
///
/// Inputs: the app handle, the system tray event, and shared action state.
///
/// Outputs: `()` after dispatching the event.
///
/// Side effects: May show the main window or exit the app.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: tray menu wiring in `app::run`.
///
/// Why this exists: keep the `tauri::Builder` wiring readable and keep the tray UX minimal.
pub(crate) fn handle_tray_event(
    app: &tauri::AppHandle,
    event: TrayIconEvent,
    state: &SharedActionState,
) {
    if state.is_quitting.load(Ordering::Relaxed) {
        return;
    }
    match event {
        TrayIconEvent::DoubleClick { .. } => show_main_window(app),
        _ => {}
    }
}

/// Summary: Routes tray menu events to application actions.
///
/// Inputs: the app handle, the menu event, and shared action state.
///
/// Outputs: `()` after dispatching the selected action.
///
/// Side effects: May show the main window or initiate application quit.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `handle_tray_event` and tray wiring in `app::run`.
///
/// Why this exists: Tauri 2 tray-menu activation is delivered via the global menu event stream.
pub(crate) fn handle_menu_event(app: &tauri::AppHandle, event: MenuEvent, state: &SharedActionState) {
    if state.is_quitting.load(Ordering::Relaxed) {
        return;
    }
    match event.id().as_ref() {
        tray::SHOW => show_main_window(app),
        tray::QUIT => confirm_and_quit(app, state.clone()),
        _ => {}
    }
}

/// Summary: Handles window events with a consistent close policy.
///
/// Inputs: the global window event and shared action state.
///
/// Outputs: `()` after handling the event.
///
/// Side effects: Prevents the default close behavior and exits the app cleanly.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: lifecycle wiring in `app::run`.
///
/// Why this exists: avoid macOS-specific close quirks leaving a blank re-opened window.
pub(crate) fn handle_window_event(
    window: &tauri::Window,
    event: &WindowEvent,
    state: &SharedActionState,
) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        let runtime = super::tuning::resolve_runtime_tuning();
        let close_final_backup_debounce =
            std::time::Duration::from_millis(runtime.gui_close_final_backup_debounce_ms.max(1));
        let close_final_backup_reset_delay = std::time::Duration::from_secs(
            runtime.gui_close_final_backup_reset_delay_seconds.max(1),
        );
        if state.is_quitting.load(Ordering::Relaxed) {
            // Allow a real quit to close the window; don't convert it into a hide-to-tray.
            return;
        }
        // Prevent the app from fully quitting on window close. The menu bar icon remains active.
        api.prevent_close();
        if let Err(error) = window.hide() {
            warn!(
                error = %error,
                "app::actions::handle_window_event failed to hide main window"
            );
        }

        // Best-effort "last backup" on window close (hide-to-tray) without blocking the UI thread.
        // Guard to avoid repeated triggers from rapid close/reopen interactions.
        if !state.close_backup_in_flight.swap(true, Ordering::Relaxed) {
            let close_state = state.clone();
            async_runtime::spawn(async move {
                // Small debounce window; allow the close/hide to complete.
                tokio::time::sleep(close_final_backup_debounce).await;
                match config_api::get_config() {
                    Ok(cfg) => {
                        if !cfg.safe_mode {
                            if let Err(error) =
                                commands::backup::run::run_now_cmd(Some("close-final".to_string()))
                                    .await
                            {
                                warn!(
                                    error = %error.message,
                                    "app::actions::handle_window_event close-final backup failed"
                                );
                            }
                        }
                    }
                    Err(error) => {
                        warn!(
                            error = %error,
                            "app::actions::handle_window_event failed loading config for close-final backup"
                        );
                    }
                }
                tokio::time::sleep(close_final_backup_reset_delay).await;
                close_state
                    .close_backup_in_flight
                    .store(false, Ordering::Relaxed);
            });
        }
    }
}

/// Summary: Ask for confirmation before quitting because quitting pauses backups.
///
/// Inputs: the Tauri app handle and shared action state.
///
/// Outputs: `()` after handling the quit flow.
///
/// Side effects: Shows a confirmation dialog, may pause backups, may stop the daemon service on macOS, and may exit the app.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: tray Quit action.
///
/// Why this exists: avoid accidental shutdown of background backups and make the consequence explicit.
fn confirm_and_quit(app: &tauri::AppHandle, state: SharedActionState) {
    if state.is_quitting.load(Ordering::Relaxed) {
        return;
    }

    let handle = app.clone();
    let confirm_state = state.clone();
    handle
        .dialog()
        .message("Quitting will stop background backups. Are you sure you want to quit?")
        .title("Quit Backup Sync?")
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Quit".to_string(),
            "Cancel".to_string(),
        ))
        .show(move |confirmed| {
            if !confirmed {
                return;
            }
            if confirm_state.is_quitting.swap(true, Ordering::Relaxed) {
                return;
            }

            // Disable tray quit item immediately to prevent duplicate clicks.
            if let Some(quit_item) = super::tray_menu::quit_handle() {
                if let Err(error) = quit_item.set_enabled(false) {
                    warn!(
                        error = %error,
                        "app::actions::confirm_and_quit failed disabling quit tray item"
                    );
                }
                if let Err(error) = quit_item.set_text("Quitting...") {
                    warn!(
                        error = %error,
                        "app::actions::confirm_and_quit failed updating quit tray item title"
                    );
                }
            }

            if let Some(window) = handle.get_webview_window("main") {
                if let Err(error) = window.hide() {
                    warn!(
                        error = %error,
                        "app::actions::confirm_and_quit failed hiding main window"
                    );
                }
            }

            async_runtime::spawn(async move {
                quit_sequence(handle).await;
            });
        });
}

/// Summary: Run one last backup before quitting with a time bound.
///
/// Inputs: None.
///
/// Outputs: `Ok(())` when the final backup completed; otherwise a user-facing failure string.
///
/// Side effects: Performs backup IO and persists state updates.
///
/// Error handling: Returns a string describing the failure so the caller can prompt the user.
///
/// Ties to other methods: Used by `quit_sequence` prior to stopping background work.
///
/// Why this exists: Avoid losing recent changes when the user quits from the menu bar.
async fn run_final_backup_best_effort(timeout: std::time::Duration) -> Result<(), String> {
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
///
/// Outputs: None.
///
/// Side effects: Pauses the daemon, optionally runs a final backup, stops the background service on macOS, closes windows, and exits.
///
/// Error handling: Prompts the user if the final backup fails and resumes scheduling if they cancel quit.
///
/// Ties to other methods: Spawned by `confirm_and_quit` after user confirmation.
///
/// Why this exists: Keep tray quit responsive and ensure a clean shutdown without re-opening the window.
async fn quit_sequence(app: tauri::AppHandle) {
    let runtime = super::tuning::resolve_runtime_tuning();
    let quit_force_exit_timeout =
        std::time::Duration::from_secs(runtime.gui_quit_force_exit_timeout_seconds.max(1));
    let quit_final_backup_timeout =
        std::time::Duration::from_secs(runtime.gui_quit_final_backup_timeout_seconds.max(1));
    let daemon_stop_timeout =
        std::time::Duration::from_secs(runtime.gui_daemon_stop_timeout_seconds.max(1));
    // Hard stop guard: ensure Quit actually terminates even if background tasks keep
    // the process alive. This is only armed after the user confirms quitting.
    std::thread::spawn(move || {
        std::thread::sleep(quit_force_exit_timeout);
        std::process::exit(0);
    });

    let cfg_safe_mode = match config_api::get_config() {
        Ok(cfg) => cfg.safe_mode,
        Err(error) => {
            warn!(
                error = %error,
                "app::actions::quit_sequence failed loading config; defaulting safe_mode=true"
            );
            true
        }
    };

    if !cfg_safe_mode {
        // Best-effort only; quitting is an explicit stop and should not hang.
        if let Err(error) = run_final_backup_best_effort(quit_final_backup_timeout).await {
            warn!(
                error = %error,
                "app::actions::quit_sequence final backup failed"
            );
        }
    }

    pause_backups_best_effort();
    stop_background_daemon_best_effort_async(daemon_stop_timeout).await;

    if let Some(window) = app.get_webview_window("main") {
        if let Err(error) = window.close() {
            warn!(
                error = %error,
                "app::actions::quit_sequence failed closing main window"
            );
        }
    }
    app.exit(0);
}

/// Summary: Pause backups before quitting by enabling safe mode in config and daemon.
///
/// Inputs: none.
///
/// Outputs: `()` after best-effort attempts complete.
///
/// Side effects: Writes configuration and attempts to update the running daemon over IPC.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `confirm_and_quit`.
///
/// Why this exists: quitting should stop background backups immediately and predictably.
fn pause_backups_best_effort() {
    let ipc_cid = commands::correlation::cid("pause-backups", None);
    match config_api::get_config() {
        Ok(mut cfg) => {
            if !cfg.safe_mode {
                cfg.safe_mode = true;
                if let Err(error) = config_api::save_config(&cfg) {
                    warn!(
                        error = %error,
                        "app::actions::pause_backups_best_effort failed saving safe_mode config"
                    );
                }
            }
        }
        Err(error) => {
            warn!(
                error = %error,
                "app::actions::pause_backups_best_effort failed loading config"
            );
        }
    }
    async_runtime::spawn(async move {
        if let Err(error) =
            status_api::set_safe_mode_with_correlation(true, Some(ipc_cid.as_str())).await
        {
            warn!(
                cid = %ipc_cid,
                error = %error,
                "app::actions::pause_backups_best_effort failed setting daemon safe mode"
            );
        }
    });
}

/// Summary: Stops the launchd-managed daemon as a best-effort cleanup during quit.
///
/// Inputs: timeout budget for launchctl stop/join flow.
///
/// Outputs: none.
///
/// Side effects: invokes `id` and `launchctl` commands and may stop the background daemon service.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `quit_sequence`.
///
/// Why this exists: keep quit semantics explicit and avoid orphaned background daemons.
#[cfg(target_os = "macos")]
async fn stop_background_daemon_best_effort_async(timeout: std::time::Duration) {
    const LABEL: &str = "com.backup_sync.daemon";

    let stop = async_runtime::spawn_blocking(move || {
        let uid_output = match run_command("id", &["-u"], "DAEMON_STOP_UID", "read uid with id -u")
        {
            Ok(output) => output,
            Err(error) => {
                warn!(
                    error = %error,
                    "app::actions::stop_background_daemon_best_effort_async failed to read uid with id -u"
                );
                return;
            }
        };
        let uid = match String::from_utf8(uid_output.stdout) {
            Ok(stdout) => stdout.trim().to_string(),
            Err(error) => {
                warn!(
                    error = %error,
                    "app::actions::stop_background_daemon_best_effort_async failed parsing uid output as utf-8"
                );
                return;
            }
        };
        if uid.is_empty() {
            warn!("app::actions::stop_background_daemon_best_effort_async resolved empty uid");
            return;
        }
        let target = format!("gui/{}/{}", uid, LABEL);
        match run_command(
            "launchctl",
            &["bootout", "-k", &target],
            "DAEMON_STOP_BOOTOUT",
            "launchctl bootout",
        ) {
            Ok(_) => {}
            Err(error) => {
                warn!(
                    target = %target,
                    error = %error,
                    "app::actions::stop_background_daemon_best_effort_async failed to execute launchctl bootout"
                );
            }
        }
    });
    match tokio::time::timeout(timeout, stop).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            warn!(
                error = %error,
                "app::actions::stop_background_daemon_best_effort_async join failed"
            );
        }
        Err(error) => {
            warn!(
                error = %error,
                "app::actions::stop_background_daemon_best_effort_async timed out"
            );
        }
    }
}

/// Summary: Non-macOS no-op for background daemon stop helper.
///
/// Inputs: timeout budget (unused outside macOS).
///
/// Outputs: none.
///
/// Side effects: none.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `quit_sequence`.
///
/// Why this exists: keep cross-platform call sites unified without cfg-splitting callers.
#[cfg(not(target_os = "macos"))]
async fn stop_background_daemon_best_effort_async(_timeout: std::time::Duration) {}
