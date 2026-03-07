use crate::tray;
use tauri::{CustomMenuItem, SystemTray, SystemTrayMenu, SystemTrayMenuItem};

/// Summary: Builds the system tray menu used by the GUI.
///
/// Inputs: none.
///
/// Outputs: a `SystemTray` with a fully populated menu.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: tray event routing in `app::actions`.
///
/// Why this exists: keep tray menu construction separate from `tauri::Builder` wiring.
pub(crate) fn build_tray() -> SystemTray {
    let tray_menu = SystemTrayMenu::new()
        .add_item(CustomMenuItem::new(tray::SHOW, "Open Backup Sync"))
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new(tray::STATUS_LINE, "Status: Checking…").disabled())
        .add_item(CustomMenuItem::new(tray::LAST_SYNC_LINE, "Last sync: —").disabled())
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new(tray::QUIT, "Quit"));
    SystemTray::new().with_menu(tray_menu)
}
