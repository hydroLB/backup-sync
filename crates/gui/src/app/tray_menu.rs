use crate::tray;
use tauri::{CustomMenuItem, SystemTray, SystemTrayMenu, SystemTrayMenuItem};

/// Purpose: Builds the system tray menu used by the GUI.
///
/// Inputs: none.
/// Outputs: a `SystemTray` with a fully populated menu.
/// Ties to: tray event routing in `app::actions`.
/// Side effects: None.
/// Why: keep tray menu construction separate from `tauri::Builder` wiring.
pub(crate) fn build_tray() -> SystemTray {
    let tray_menu = SystemTrayMenu::new()
        .add_item(CustomMenuItem::new(tray::SHOW, "Show Backup Sync"))
        .add_item(CustomMenuItem::new(tray::RUN_NOW, "Run backup now"))
        .add_item(CustomMenuItem::new(tray::VERIFY, "Verify backups"))
        .add_item(CustomMenuItem::new(
            tray::INSTALL,
            "Start on login (install service)",
        ))
        .add_item(CustomMenuItem::new(
            tray::EXPORT_LOGS,
            "Export logs to Desktop",
        ))
        .add_item(CustomMenuItem::new(
            tray::CHECK_UPDATES,
            "Check for updates",
        ))
        .add_item(CustomMenuItem::new(
            tray::EXPORT_HEALTH,
            "Export health report",
        ))
        .add_item(CustomMenuItem::new(tray::DOCTOR, "Run doctor (export)"))
        .add_item(CustomMenuItem::new(tray::RESTART, "Restart daemon"))
        .add_item(CustomMenuItem::new(
            tray::TOGGLE_SAFE_MODE,
            "Toggle safe mode",
        ))
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new(tray::QUIT, "Quit"));
    SystemTray::new().with_menu(tray_menu)
}
