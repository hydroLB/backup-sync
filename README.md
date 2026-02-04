# Backup Sync

Backup Sync is a simple desktop backup app for space efficient, versioned backups of selected folders to an external drive or partition. It uses a content addressed blob store (SHA-256) so unchanged file contents are not duplicated across versions.

## Repository map
- `crates/core` contains the backup engine, blob store layout, and restore logic
- `crates/daemon` runs scheduled cycles, watcher integrations, IPC, and shutdown handling
- `crates/cli` provides operational commands for status, run once, verify, and diagnostics
- `crates/gui` is the Tauri backend and IPC bridge
- `crates/gui/frontend` is the React UI and service layer
- `docs` holds standards, perf baselines, and release notes

## System overview
- By default every 30 minutes (configurable), the daemon scans each configured folder and builds a snapshot manifest.
- A new version is created only when the snapshot differs from the latest manifest (add, modify, delete).
- File contents are stored in `.backup_sync/v1/blobs/sha256/...` keyed by SHA-256; manifests map relative paths to blob hashes plus metadata.
- Each folder keeps N versions (default 5). When retention prunes old manifests, unreferenced blobs are garbage collected.
- Restore reconstructs a selected version either to a new directory or in place (in place removes files not present in the selected version).

Details: see `docs/storage.md` and `docs/ui.md`.

## Setup
1. Install Rust stable, Node 18 or newer, and npm.
2. Enable git hooks with `make hooks`.
3. Build everything with `make build`.
4. Optional: set `BACKUP_SYNC_PASSPHRASE` to enable session locking for privileged actions, for example `export BACKUP_SYNC_PASSPHRASE="change-me"`.
5. Run the desktop app with `make run`.

Quickstart (dev):
- `./start` (builds prerequisites and launches the desktop app)

## Usage

### Desktop app
1. Start the app with `make run`.
2. Choose a backup destination (external drive/partition path).
3. Add one or more folders to back up.
4. Set “Backups to keep” per folder (default 5).
5. Use the Running toggle to pause or resume background writes (safe mode).
6. Adjust the schedule via the interval (minutes) control.
7. Use Restore version to pick a folder + version and restore to a new directory or in place.

### CLI
- Status snapshot: `cargo run -p cli -- status`
- Guided setup: `cargo run -p cli -- init`
- One off backup: `cargo run -p cli -- run-once` with `--dry-run`
- Verify integrity: `cargo run -p cli -- verify`
- Diagnostics: `cargo run -p cli -- doctor`
- Install service on login: `cargo run -p cli -- install-service --enable`

## Configuration

Config file path:
- `~/.config/backup_sync/config.toml`

State file path:
- `~/.config/backup_sync/state.json`

UI tuning knobs live in `crates/gui/frontend/src/config/uiTuning.ts`.
Backend config defaults and guardrails are centralized in `crates/core/src/config/registry.rs`.

Auth passphrase:
- If `BACKUP_SYNC_PASSPHRASE` is set and non-empty, the Settings UI will require it to unlock privileged actions.
- If unset, session locking is disabled and privileged actions are available without an unlock step.

Security notes:
- Run the daemon as your user account and avoid elevated privileges unless required by your platform.
- Store `BACKUP_SYNC_PASSPHRASE` in a secure environment file or secret manager, not in the repo.

Example config:
```toml
backup_root = "/Users/me/Backups"
interval_seconds = 1800
max_backups_per_file = 5
ignore_patterns = ["**/node_modules/**", "**/target/**", "**/.DS_Store"]
safe_mode = false

[hashing]
buffer_bytes = 65536
timeout_seconds = 30

[execution]
copy_buffer_bytes = 65536
copy_timeout_seconds = 300
free_space_safety_buffer_bytes = 10485760
recent_activity_cap = 50
retry_delays_ms = [100, 200, 400, 800]
retry_jitter_pct = 0.2

[planning]
hash_check_interval = 5
max_plan_items = 20000
scan_timeout_seconds = 300
scan_capacity_multiplier = 16

[runtime]
prune_interval_cycles = 10
verify_interval_seconds = 86400
watcher_debounce_seconds = 2
ipc_timeout_seconds = 5
service_command_timeout_seconds = 15
service_command_retry_delay_ms = 300
service_command_poll_interval_ms = 50
auth_unlock_seconds = 900
gui_start_hidden = false
tray_tooltip_refresh_seconds = 10
log_tail_lines = 200
simulation_sample_limit = 10

[[destinations]]
id = "primary"
path = "/Users/me/Backups"
label = "Primary"

[[watched]]
path = "/Users/me/Projects"
kind = "Directory"
enabled = true
destination_id = "primary"
max_backups_per_file = 10
```

## Performance
- Performance baselines are recorded by `scripts/perf/record_baseline.sh` into `docs/perf-baseline.json`.
- Regression checks run via `scripts/perf/check_baseline.sh` and are enforced in CI.
- Hot paths are exercised in `crates/core/src/bin/perf_guard.rs`.

## Testing
- Backend unit and integration tests: `cargo test -p backup_core -p daemon -p cli`
- Frontend unit tests: `npm --prefix crates/gui/frontend test`
- End to end smoke test: `cargo test -p backup_core --test e2e_smoke`
- Format and lint: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, `npm --prefix crates/gui/frontend run lint`
- Perf guard: `make perf-check`
- Full quality gate run: `make ci` (format, lint, typecheck, tests with coverage, perf guard, security audits)

## Troubleshooting
- If the UI cannot connect, ensure the daemon is running and the IPC socket is reachable.
- If backups are skipped, verify watched paths exist and are outside the destination.
- If verification reports issues, run `backup-sync verify` and review recent logs.
- For detailed diagnostics, export a doctor report from the UI or CLI.
- If the dev server port conflicts with another program, set `BACKUP_SYNC_DEV_PORT` or let Vite pick the repo-specific default in `crates/gui/frontend/vite.config.ts`.

## Release notes
- Changes are tracked in `CHANGELOG.md`.
- Versioning and tagging guidance lives in `docs/release.md`.

## Notable code paths
This system emphasizes bounded IO, explicit config validation, and performance guardrails that stay visible in code review.
- `crates/core/src/backup/versioned/store.rs` implements the SHA-256 blob store, manifests, retention, and garbage collection.
- `crates/core/src/backup/versioned/restore.rs` reconstructs versions with atomic writes and optional in-place cleanup.
- `crates/core/src/config/registry.rs` centralizes defaults and validation limits for config knobs.
- `crates/core/src/bin/perf_guard.rs` captures and validates performance baselines.
- `crates/daemon/src/runtime/loop.rs` coordinates lifecycle, IPC, and graceful shutdown.
- `crates/gui/frontend/src/config/uiTuning.ts` consolidates UI tuning knobs.
- `crates/gui/frontend/src/components/minimal/MinimalMain.tsx` is the spec-minimal main screen UI.
