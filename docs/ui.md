# Desktop UI Contract

Last updated: 2026-08-23

The Backup Sync desktop app exposes the common recovery workflow while leaving advanced storage and runtime tuning in `config.toml`.

## Main screen

- Running toggle: maps to safe mode and pauses/resumes background writes.
- Schedule summary: fixed at every 30 minutes; the UI does not present a custom interval editor.
- Destination picker: selects the primary external drive or mounted path.
- Protected folders: add/remove paths and set retained versions per source.
- Health/log details: show destination, daemon, safety, and recent activity signals.
- Restore: explicitly search a folder's versions, choose destination/in-place mode, and confirm the operation.

Enabling Running is blocked until hardening checks confirm source access, destination writability, and minimum free space. Destination loss pauses writes and surfaces a warning.

## Failure and request behavior

- Invalid or unreadable config renders a recoverable error with Retry; the native shell stays open.
- The application-level error boundary gives recovery guidance without claiming that no operation started.
- Restore requests are versioned so an older response cannot replace newer source/version choices.
- Changing scope, source, or version clears dependent selections.
- Dialog bodies scroll within the viewport, controls have accessible names/state, and reduced-motion preferences are honored.

## Lifecycle

- Closing the main window hides it.
- Close does not run a backup, change safe mode, or persist configuration.
- Quitting closes the GUI and leaves an installed background daemon service running.
- Quit does not run an implicit backup, stop the daemon, persist safe mode, or force-exit after a timer.

## Component map

- `crates/gui/frontend/src/components/minimal/MinimalMain.tsx`: status, destination, folders, log, restore entry
- `crates/gui/frontend/src/components/minimal/RestoreModal.tsx`: explicit version search and restore choices
- `crates/gui/frontend/src/components/AppErrorBoundary.tsx`: last-resort UI recovery boundary
- `crates/gui/frontend/src/services`: typed Tauri command wrappers
- `crates/gui/src/commands`: blocking-task command adapters and stable boundary errors

Advanced interval, replication, compression, encryption, scrub, and snapshot settings are documented in [Configuration](configuration.md).
