use backup_core::logging::redact_text;
use tauri::async_runtime;
use tauri::Manager;
use tracing::{error, warn};

mod actions;
mod tray_menu;
mod tray_tooltip;
mod tuning;

/// Centralize GUI wiring in one launch routine that is shared by all binaries.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Log panics with correlation metadata so crash reports can be grouped.
    let panic_cid = crate::commands::correlation::cid("gui-panic", None);
    let startup_cid = crate::commands::correlation::cid("gui-run", None);
    std::panic::set_hook(Box::new(move |info| {
        let panic_text = redact_text(&info.to_string());
        error!(
            cid = %panic_cid,
            action = "panic",
            panic = %panic_text,
            "gui panic"
        );
    }));

    let runtime = tuning::resolve_runtime_tuning();
    let tray_refresh = std::time::Duration::from_secs(runtime.tray_tooltip_refresh_seconds);
    let action_state = actions::new_action_state();

    let builder = tauri::Builder::default().plugin(tauri_plugin_dialog::init());

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
            let setup_cid = startup_cid.clone();
            let tray_handles = tray_menu::build_tray(app)?;
            tray_menu::register_handles(&tray_handles);
            if let Some(tray) = app.tray_by_id("main") {
                if let Err(error) = tray.set_menu(Some(tray_handles.menu.clone())) {
                    warn!(
                        cid = %setup_cid,
                        error = %error,
                        "app::run failed to attach tray menu during startup"
                    );
                }
            } else {
                warn!(cid = %setup_cid, "app::run could not find configured tray icon");
            }
            // Keep tray tooltip updated with daemon status.
            let handle = app.handle().clone();
            async_runtime::spawn(async move {
                loop {
                    tray_tooltip::update_tray_tooltip(&handle).await;
                    tokio::time::sleep(tray_refresh).await;
                }
            });

            // A packaged desktop app must be functional without a separate CLI install step.
            // The bundled daemon is adjacent to the GUI executable and remains managed by the
            // existing restart path (launchd when installed, direct child process otherwise).
            async_runtime::spawn_blocking(|| {
                if let Err(error) = crate::commands::service::restart_daemon_cmd(None) {
                    warn!(
                        error = %error.message,
                        "app::run could not start the bundled background service"
                    );
                }
            });

            if runtime.gui_start_hidden {
                // Hide on launch for users that want tray-first behavior.
                if let Some(window) = app.get_webview_window("main") {
                    if let Err(error) = window.hide() {
                        warn!(
                            cid = %setup_cid,
                            error = %error,
                            "app::run failed to hide main window during startup"
                        );
                    }
                }
            } else {
                actions::show_main_window(app.app_handle());
            }
            Ok(())
        })
        .on_menu_event({
            let action_state = action_state.clone();
            move |app, event| actions::handle_menu_event(app, event, &action_state)
        })
        .on_tray_icon_event({
            let action_state = action_state.clone();
            move |app, event| actions::handle_tray_event(app, event, &action_state)
        })
        .on_window_event({
            let action_state = action_state.clone();
            move |window, event| actions::handle_window_event(window, event, &action_state)
        })
        .invoke_handler(tauri::generate_handler![
            crate::commands::status::get_status,
            crate::commands::config::load_config_cmd,
            crate::commands::config::save_config_cmd,
            crate::commands::config::remove_protected_path_cmd,
            crate::commands::backup::run::run_now_cmd,
            crate::commands::backup::run::run_simulate_cmd,
            crate::commands::backup::restore::list_versions_cmd,
            crate::commands::backup::restore::list_version_files_cmd,
            crate::commands::backup::restore::restore_version_cmd,
            crate::commands::backup::restore::restore_files_cmd,
            crate::commands::backup::verify::verify_cmd,
            crate::commands::logs::log_tail_cmd,
            crate::commands::service::install_service_cmd,
            crate::commands::service::check_service_cmd,
            crate::commands::logs::export_logs_cmd,
            crate::commands::updates::check_updates_cmd,
            crate::commands::backup::verify::export_health_report_cmd,
            crate::commands::service::restart_daemon_cmd,
            crate::commands::destination::check_destination_cmd,
            crate::commands::destination::open_destination_cmd,
            crate::commands::destination::relocate_destination_cmd,
            crate::commands::access::test_access_cmd,
            crate::commands::support::diagnostics::doctor_report_cmd,
            crate::commands::support::diagnostics::export_diagnostic_bundle_cmd,
            crate::commands::config::toggle_safe_mode_cmd,
            crate::commands::hardening::hardening_check_cmd,
            crate::commands::backup::safety::remove_kept_extra_version_cmd
        ])
        .run(tauri::generate_context!())?;
    Ok(())
}
