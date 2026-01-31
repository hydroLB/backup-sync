use crate::commands::auth::SessionAuth;
use tauri::async_runtime;
use tauri::Manager;
use tauri_plugin_single_instance::init as single_instance;

mod actions;
mod tray_menu;
mod tray_tooltip;
mod tuning;

/// Purpose: Builds and runs the Tauri application with tray and command wiring.
///
/// Inputs: none.
/// Outputs: `Ok(())` when the Tauri runtime exits cleanly.
/// Ties to: GUI startup and lifecycle hooks.
/// Side effects: Registers panic hooks, spawns background tasks, and starts the GUI runtime.
/// Why: centralize GUI wiring in one launch routine that is shared by all binaries.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Log panics to stderr so unexpected failures are visible outside the UI.
    std::panic::set_hook(Box::new(|info| {
        eprintln!("[panic] {info}");
    }));

    let runtime = tuning::resolve_runtime_tuning();
    let tray_refresh = std::time::Duration::from_secs(runtime.tray_tooltip_refresh_seconds);

    tauri::Builder::default()
        .plugin(single_instance(|app, _argv, _cwd| {
            // Focus existing instance instead of spawning a second tray/window.
            actions::show_main_window(app);
        }))
        .manage(SessionAuth::default())
        .setup(move |app| {
            // Keep tray tooltip updated with daemon status.
            let handle = app.app_handle();
            async_runtime::spawn(async move {
                loop {
                    tray_tooltip::update_tray_tooltip(&handle).await;
                    tokio::time::sleep(tray_refresh).await;
                }
            });

            // Start fully hidden to keep background behavior predictable.
            if let Some(window) = app.get_window("main") {
                let _ = window.hide();
            }
            Ok(())
        })
        .system_tray(tray_menu::build_tray())
        .on_system_tray_event(|app, event| actions::handle_tray_event(app, event))
        .on_window_event(actions::handle_window_event)
        .invoke_handler(tauri::generate_handler![
            crate::commands::status::get_status,
            crate::commands::config::load_config_cmd,
            crate::commands::config::save_config_cmd,
            crate::commands::backup::run::run_now_cmd,
            crate::commands::backup::run::run_simulate_cmd,
            crate::commands::backup::verify::verify_cmd,
            crate::commands::logs::log_tail_cmd,
            crate::commands::service::install_service_cmd,
            crate::commands::service::check_service_cmd,
            crate::commands::logs::export_logs_cmd,
            crate::commands::updates::check_updates_cmd,
            crate::commands::backup::verify::export_health_report_cmd,
            crate::commands::service::restart_daemon_cmd,
            crate::commands::destination::check_destination_cmd,
            crate::commands::access::test_access_cmd,
            crate::commands::support::diagnostics::doctor_report_cmd,
            crate::commands::support::diagnostics::export_diagnostic_bundle_cmd,
            crate::commands::auth::unlock_session_cmd,
            crate::commands::auth::lock_session_cmd,
            crate::commands::auth::auth_status_cmd,
            crate::commands::config::toggle_safe_mode_cmd
        ])
        .run(tauri::generate_context!())?;
    Ok(())
}
