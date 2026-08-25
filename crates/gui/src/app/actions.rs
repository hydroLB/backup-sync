use crate::tray;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::menu::MenuEvent;
use tauri::tray::TrayIconEvent;
use tauri::{Manager, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use tracing::warn;

/// Replace module-level global statics with an injected runtime state object.
#[derive(Default)]
pub(crate) struct ActionState {
    is_quitting: AtomicBool,
}

pub(crate) type SharedActionState = Arc<ActionState>;

/// Make lifecycle state explicit and dependency-injected.
pub(crate) fn new_action_state() -> SharedActionState {
    Arc::new(ActionState::default())
}

/// Provide a consistent way to surface the UI.
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

/// Keep the `tauri::Builder` wiring readable and keep the tray UX minimal.
pub(crate) fn handle_tray_event(
    app: &tauri::AppHandle,
    event: TrayIconEvent,
    state: &SharedActionState,
) {
    if state.is_quitting.load(Ordering::Relaxed) {
        return;
    }
    if let TrayIconEvent::DoubleClick { .. } = event {
        show_main_window(app);
    }
}

/// Tauri 2 tray-menu activation is delivered via the global menu event stream.
pub(crate) fn handle_menu_event(
    app: &tauri::AppHandle,
    event: MenuEvent,
    state: &SharedActionState,
) {
    if state.is_quitting.load(Ordering::Relaxed) {
        return;
    }
    match event.id().as_ref() {
        tray::SHOW => show_main_window(app),
        tray::QUIT => confirm_and_quit(app, state.clone()),
        _ => {}
    }
}

/// Avoid macOS-specific close quirks leaving a blank re-opened window.
pub(crate) fn handle_window_event(
    window: &tauri::Window,
    event: &WindowEvent,
    state: &SharedActionState,
) {
    if let WindowEvent::CloseRequested { api, .. } = event {
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
    }
}

/// Confirm that only the desktop UI should exit while daemon-owned backups continue.
fn confirm_and_quit(app: &tauri::AppHandle, state: SharedActionState) {
    if state.is_quitting.load(Ordering::Relaxed) {
        return;
    }

    let handle = app.clone();
    let confirm_state = state.clone();
    handle
        .dialog()
        .message(
            "The desktop UI will exit. Backups managed by an installed background service will continue. Are you sure you want to quit?",
        )
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
                if let Err(error) = window.close() {
                    warn!(
                        error = %error,
                        "app::actions::confirm_and_quit failed closing main window"
                    );
                }
            }
            handle.exit(0);
        });
}
