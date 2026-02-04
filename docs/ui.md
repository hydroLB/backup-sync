# UI (Wireframe and Components)

## Wireframe

Main screen:

- Status
  - Running toggle (pauses/resumes background writes using safe mode)
  - Interval (minutes) editor for the scheduled scan cadence

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

- Optional
  - Show log (collapsible log tail)
  - Restore version button (opens folder + version restore flow)

## Component list

Frontend:
- `crates/gui/frontend/src/components/minimal/MinimalMain.tsx`
  - Destination selector (Tauri directory picker)
  - Folder list editor (add/remove + keep versions per folder)
  - Running toggle and interval editor
  - Restore version action
  - Log tail (hidden until toggled)
- `crates/gui/frontend/src/components/minimal/RestoreModal.tsx`
  - Folder selector
  - Version selector
  - Restore mode selector (to directory vs in place)

Backend commands:
- `load_config_cmd`, `save_config_cmd`
- `list_versions_cmd`, `restore_version_cmd`
- `toggle_safe_mode_cmd` (also updates the running daemon when reachable)
