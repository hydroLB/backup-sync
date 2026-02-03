use crate::commands;
use crate::commands::auth::SessionAuth;
use crate::tray;
use tauri::async_runtime;
use tauri::{Manager, SystemTrayEvent, WindowEvent};

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

/// Purpose: Handles tray events by routing to the appropriate async actions.
///
/// Inputs: the app handle and the system tray event.
/// Outputs: `()` after dispatching the event.
/// Ties to: tray menu wiring in `app::run`.
/// Side effects: May spawn async tasks and may exit the app.
/// Why: keep the `tauri::Builder` wiring readable while keeping actions testable.
pub(crate) fn handle_tray_event(app: &tauri::AppHandle, event: SystemTrayEvent) {
    match event {
        SystemTrayEvent::LeftClick { .. } => show_main_window(app),
        SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
            tray::SHOW => show_main_window(app),
            tray::RUN_NOW => spawn_run_now(app),
            tray::VERIFY => spawn_verify(),
            tray::INSTALL => spawn_install_service(app),
            tray::EXPORT_LOGS => spawn_export_logs(),
            tray::CHECK_UPDATES => spawn_updates(),
            tray::EXPORT_HEALTH => spawn_export_health(),
            tray::DOCTOR => spawn_doctor(app),
            tray::RESTART => spawn_restart(app),
            tray::TOGGLE_SAFE_MODE => spawn_toggle_safe_mode(app),
            tray::QUIT => app.exit(0),
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
        // Prevent Tauri/macOS from just hiding the window; force a full quit so it does not re-open blank.
        api.prevent_close();
        // Ask Tauri to terminate cleanly so we don't leave stray tray icons/processes.
        event.window().app_handle().exit(0);
    }
}

/// Purpose: Spawns an async task to run a backup immediately.
///
/// Inputs: the Tauri app handle.
/// Outputs: `()` after spawning the task.
/// Ties to: tray run now actions.
/// Side effects: Triggers a backup run and logs failures to stderr.
/// Why: keep tray actions non-blocking.
fn spawn_run_now(app: &tauri::AppHandle) {
    let handle = app.app_handle();
    async_runtime::spawn(async move {
        let auth = handle.state::<SessionAuth>();
        if let Err(e) = commands::backup::run::run_now_cmd(None, auth).await {
            eprintln!("run_now tray action failed: {:?}", e);
        }
    });
}

/// Purpose: Spawns an async task to verify backups.
///
/// Inputs: none.
/// Outputs: `()` after spawning the task.
/// Ties to: tray verify actions.
/// Side effects: Triggers verification and logs failures to stderr.
/// Why: keep verification off the main thread.
fn spawn_verify() {
    async_runtime::spawn(async move {
        if let Err(e) = commands::backup::verify::verify_cmd(None).await {
            eprintln!("verify tray action failed: {:?}", e);
        }
    });
}

/// Purpose: Spawns an async task to install the background service.
///
/// Inputs: the Tauri app handle.
/// Outputs: `()` after spawning the task.
/// Ties to: tray service installation actions.
/// Side effects: Runs service installation commands and logs failures.
/// Why: keep service setup off the UI thread.
fn spawn_install_service(app: &tauri::AppHandle) {
    let handle = app.app_handle();
    async_runtime::spawn(async move {
        let auth = handle.state::<SessionAuth>();
        if let Err(e) = commands::service::install_service_cmd(None, auth).await {
            eprintln!("install service failed: {:?}", e);
        }
    });
}

/// Purpose: Spawns an async task to export logs.
///
/// Inputs: none.
/// Outputs: `()` after spawning the task.
/// Ties to: tray export logs actions.
/// Side effects: Writes an exported log file and logs failures.
/// Why: keep log export non-blocking.
fn spawn_export_logs() {
    async_runtime::spawn(async move {
        if let Err(e) = commands::logs::export_logs_cmd() {
            eprintln!("export logs failed: {:?}", e);
        }
    });
}

/// Purpose: Spawns an async task to check for updates.
///
/// Inputs: none.
/// Outputs: `()` after spawning the task.
/// Ties to: tray update check actions.
/// Side effects: Reads the update feed file and logs failures.
/// Why: keep update checks asynchronous.
fn spawn_updates() {
    async_runtime::spawn(async move {
        if let Err(e) = commands::updates::check_updates_cmd().await {
            eprintln!("update check failed: {:?}", e);
        }
    });
}

/// Purpose: Spawns an async task to export a health report.
///
/// Inputs: none.
/// Outputs: `()` after spawning the task.
/// Ties to: tray health report actions.
/// Side effects: Writes a health report file and logs failures.
/// Why: keep report generation asynchronous.
fn spawn_export_health() {
    async_runtime::spawn(async move {
        if let Err(e) = commands::backup::verify::export_health_report_cmd(None).await {
            eprintln!("export health failed: {:?}", e);
        }
    });
}

/// Purpose: Spawns an async task to generate a doctor report.
///
/// Inputs: the Tauri app handle.
/// Outputs: `()` after spawning the task.
/// Ties to: tray diagnostics actions.
/// Side effects: Writes a doctor report file and logs failures.
/// Why: keep diagnostics export off the UI thread.
fn spawn_doctor(app: &tauri::AppHandle) {
    let handle = app.app_handle();
    async_runtime::spawn(async move {
        let auth = handle.state::<SessionAuth>();
        if let Err(e) = commands::support::diagnostics::doctor_report_cmd(None, auth).await {
            eprintln!("doctor report failed: {:?}", e);
        }
    });
}

/// Purpose: Spawns an async task to restart the daemon.
///
/// Inputs: the Tauri app handle.
/// Outputs: `()` after spawning the task.
/// Ties to: tray restart actions.
/// Side effects: Runs restart commands and logs failures.
/// Why: keep restart operations asynchronous.
fn spawn_restart(app: &tauri::AppHandle) {
    let handle = app.app_handle();
    async_runtime::spawn(async move {
        let auth = handle.state::<SessionAuth>();
        if let Err(e) = commands::service::restart_daemon_cmd(None, auth) {
            eprintln!("restart daemon failed: {:?}", e);
        }
    });
}

/// Purpose: Spawns a blocking task to toggle safe mode.
///
/// Inputs: the Tauri app handle.
/// Outputs: `()` after spawning the task.
/// Ties to: tray safe mode toggle actions.
/// Side effects: Writes the config file to toggle safe mode.
/// Why: isolate config writes from the UI thread.
fn spawn_toggle_safe_mode(app: &tauri::AppHandle) {
    let handle = app.app_handle();
    async_runtime::spawn(async move {
        let auth = handle.state::<SessionAuth>();
        if let Err(e) = commands::config::toggle_safe_mode_cmd(None, None, auth).await {
            eprintln!("toggle safe mode failed: {:?}", e);
        }
    });
}
