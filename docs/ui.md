# UI (Wireframe and Components)

## Wireframe

Main screen:

- Destination
  - Current destination path (external drive or partition)
  - Button: Choose…

- Folders
  - List of folders being backed up
  - Per folder:
    - Path
    - “Backups to keep” (default 5)
    - Remove
  - Button: Add folder…

- Actions
  - Start/Stop (pause or resume background writes using safe mode)
  - Back up now
  - Restore…

- Optional
  - Show log (collapsible log tail)

## Component list

Frontend:
- `crates/gui/frontend/src/components/minimal/MinimalMain.tsx`
  - Destination selector (Tauri directory picker)
  - Folder list editor (add/remove + keep versions per folder)
  - Start/Stop, Back up now, Restore actions
  - Log tail (hidden until toggled)
- `crates/gui/frontend/src/components/minimal/RestoreModal.tsx`
  - Folder selector
  - Version selector
  - Restore mode selector (to directory vs in place)

Backend commands:
- `load_config_cmd`, `save_config_cmd`
- `run_now_cmd`
- `list_versions_cmd`, `restore_version_cmd`
- `toggle_safe_mode_cmd` (also updates the running daemon when reachable)

