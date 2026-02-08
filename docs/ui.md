# UI (Wireframe and Components)

## Wireframe

Main screen:

- Status
  - Running toggle (pauses/resumes background writes using safe mode)
  - Interval (minutes) editor for the scheduled scan cadence
  - First-run hardening: enabling Running is blocked until safety checks confirm watched path access, destination writability, and minimum free space. Optional snapshot probing can be run from the same flow.
  - Destination health: if the destination drive is disconnected or unavailable, the daemon pauses writes, surfaces a warning, and auto-resumes when the destination returns.

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
  - Hardening wizard modal (safety checks gate running)
  - Restore version action
  - Log tail (hidden until toggled)
- `crates/gui/frontend/src/components/minimal/HardeningModal.tsx`
  - Runs hardening checks (permissions, free space, optional snapshots)
- `crates/gui/frontend/src/components/minimal/RestoreModal.tsx`
  - Folder selector
  - Version selector
  - Restore mode selector (to directory vs in place)

Backend commands:
- `load_config_cmd`, `save_config_cmd`
- `list_versions_cmd`, `restore_version_cmd`
- `toggle_safe_mode_cmd` (also updates the running daemon when reachable)
- `hardening_check_cmd` (first-run safety checks before enabling scheduling)
