use crate::tray;
use std::sync::OnceLock;
use tauri::menu::{Menu, MenuItem, MenuItemBuilder, PredefinedMenuItem};

pub(crate) struct TrayMenuHandles<R: tauri::Runtime> {
    pub(crate) menu: Menu<R>,
    pub(crate) status_line: MenuItem<R>,
    pub(crate) last_sync_line: MenuItem<R>,
    pub(crate) quit: MenuItem<R>,
}

static STATUS_LINE: OnceLock<MenuItem<tauri::Wry>> = OnceLock::new();
static LAST_SYNC_LINE: OnceLock<MenuItem<tauri::Wry>> = OnceLock::new();
static QUIT: OnceLock<MenuItem<tauri::Wry>> = OnceLock::new();

/// Tauri 2 tray menus are regular menu resources and their items must be retained explicitly.
pub(crate) fn build_tray<R: tauri::Runtime, M: tauri::Manager<R>>(
    manager: &M,
) -> tauri::Result<TrayMenuHandles<R>> {
    let show = MenuItemBuilder::with_id(tray::SHOW, "Open Backup Sync").build(manager)?;
    let status_line = MenuItemBuilder::with_id(tray::STATUS_LINE, "Status: Checking...")
        .enabled(false)
        .build(manager)?;
    let last_sync_line = MenuItemBuilder::with_id(tray::LAST_SYNC_LINE, "Last sync: -")
        .enabled(false)
        .build(manager)?;
    let quit = MenuItemBuilder::with_id(tray::QUIT, "Quit").build(manager)?;
    let separator_one = PredefinedMenuItem::separator(manager)?;
    let separator_two = PredefinedMenuItem::separator(manager)?;
    let menu = Menu::with_items(
        manager,
        &[
            &show,
            &separator_one,
            &status_line,
            &last_sync_line,
            &separator_two,
            &quit,
        ],
    )?;

    Ok(TrayMenuHandles {
        menu,
        status_line,
        last_sync_line,
        quit,
    })
}

/// Tray status updates happen outside the original setup scope.
pub(crate) fn register_handles(handles: &TrayMenuHandles<tauri::Wry>) {
    let _ = STATUS_LINE.set(handles.status_line.clone());
    let _ = LAST_SYNC_LINE.set(handles.last_sync_line.clone());
    let _ = QUIT.set(handles.quit.clone());
}

/// Tray status updates need a stable handle lookup point.
pub(crate) fn status_line_handle() -> Option<&'static MenuItem<tauri::Wry>> {
    STATUS_LINE.get()
}

/// The tray updater rewrites the label on every refresh.
pub(crate) fn last_sync_line_handle() -> Option<&'static MenuItem<tauri::Wry>> {
    LAST_SYNC_LINE.get()
}

/// Quit flow needs to disable duplicate tray interactions.
pub(crate) fn quit_handle() -> Option<&'static MenuItem<tauri::Wry>> {
    QUIT.get()
}
