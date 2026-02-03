# Backup Sync

Built as a local first backup manager with a Rust core, a long running daemon, a CLI, and a Tauri plus React desktop app. The design keeps state on disk, uses OS config and data directories, and exposes the same operations in GUI and CLI flows.

## Repository map
- `crates/core` contains the backup engine, hashing, retention policies, and state model
- `crates/daemon` runs scheduled cycles, watcher integrations, IPC, and shutdown handling
- `crates/cli` provides operational commands for status, run once, verify, and diagnostics
- `crates/gui` is the Tauri backend and IPC bridge
- `crates/gui/frontend` is the React UI and service layer
- `docs` holds standards, perf baselines, and release notes

## System overview
- A scan collects metadata for watched files, applies ignore patterns, and enforces timeouts
- Planning decides what to back up based on size, timestamps, and periodic content hashes
- Execution copies to a temp file, verifies size, then renames atomically and updates state
- Verification re hashes the newest backups and records results for the UI
- IPC exposes status snapshots with bounded payload sizes

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
2. Add at least one watched path and a destination.
3. Run a Simulation to preview changes, then run a backup.

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

## Release notes
- Changes are tracked in `CHANGELOG.md`.
- Versioning and tagging guidance lives in `docs/release.md`.

## Notable code paths
This system emphasizes bounded IO, explicit config validation, and performance guardrails that stay visible in code review.
- `crates/core/src/backup/execution/worker.rs` shows staged copy, retries with jitter, throttling, and state updates.
- `crates/core/src/config/registry.rs` centralizes defaults and validation limits for config knobs.
- `crates/core/src/fs/scanning/collector.rs` enforces scan timeouts and uses redacted logging.
- `crates/core/src/bin/perf_guard.rs` captures and validates performance baselines.
- `crates/daemon/src/runtime/loop.rs` coordinates lifecycle, IPC, and graceful shutdown.
- `crates/gui/frontend/src/config/uiTuning.ts` consolidates UI tuning knobs.
- `crates/gui/frontend/src/utils/bst.ts` keeps watch list prefix checks fast and deterministic.
