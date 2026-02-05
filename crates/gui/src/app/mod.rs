use tauri::async_runtime;
use tauri::Manager;

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

    let builder = tauri::Builder::default();

    #[cfg(feature = "single-instance")]
    let builder = {
        use tauri_plugin_single_instance::init as single_instance;
        builder.plugin(single_instance(|app, _argv, _cwd| {
            // Focus existing instance instead of spawning a second tray/window.
            actions::show_main_window(app);
        }))
    };

    builder
        .setup(move |app| {
            // Keep tray tooltip updated with daemon status.
            let handle = app.app_handle();
            async_runtime::spawn(async move {
                loop {
                    tray_tooltip::update_tray_tooltip(&handle).await;
                    tokio::time::sleep(tray_refresh).await;
                }
            });

            if runtime.gui_start_hidden {
                // Hide on launch for users that want tray-first behavior.
                if let Some(window) = app.get_window("main") {
                    let _ = window.hide();
                }
            } else {
                actions::show_main_window(&app.app_handle());
            }
            Ok(())
        })
        .system_tray(tray_menu::build_tray())
        .on_system_tray_event(actions::handle_tray_event)
        .on_window_event(actions::handle_window_event)
        .invoke_handler(tauri::generate_handler![
            crate::commands::status::get_status,
            crate::commands::config::load_config_cmd,
            crate::commands::config::save_config_cmd,
            crate::commands::backup::run::run_now_cmd,
            crate::commands::backup::run::run_simulate_cmd,
            crate::commands::backup::restore::list_versions_cmd,
            crate::commands::backup::restore::restore_version_cmd,
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
            crate::commands::config::toggle_safe_mode_cmd
        ])
        .run(tauri::generate_context!())?;
    Ok(())
}
