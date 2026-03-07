# Backup Sync

Backup Sync is a simple desktop backup app for space efficient, versioned backups of selected folders to an external drive or partition. It uses a content addressed blob store (SHA-256) so unchanged file contents are not duplicated across versions.

## Repository map
- `crates/core` contains the backup engine, blob store layout, and restore logic
- `crates/daemon` runs scheduled cycles, watcher integrations, IPC, and shutdown handling
- `crates/cli` provides operational commands for status, run once, verify, and diagnostics
- `crates/gui` is the Tauri backend and IPC bridge
- `crates/gui/frontend` is the React UI and service layer
- `docs` holds standards, perf baselines, and release notes

Note on engines:
- The production workflow uses the **versioned engine** under `crates/core/src/backup/versioned`.
- The original scan plan execute file copy engine remains in `crates/core/src/backup/{planning,execution,retention}` for internal tests and reference, but the daemon, CLI, and GUI run on the versioned engine.

## System overview
- By default every 30 minutes (configurable), the daemon scans each configured folder and builds a snapshot manifest.
- A new version is created only when the snapshot differs from the latest manifest (add, modify, delete).
- File contents are stored in `.backup_sync/v1/blobs/sha256/...` keyed by SHA-256; manifests map relative paths to blob hashes plus metadata.
- Each folder keeps N versions (default 5). When retention prunes old manifests, unreferenced blobs are garbage collected.
- Restore reconstructs a selected version either to a new directory or in place (in place removes files not present in the selected version).
- When the backup destination is disconnected or unavailable, the daemon pauses writes, reports the issue, and auto-resumes when the destination returns.

Details: see [`docs/storage.md`](docs/storage.md) and [`docs/ui.md`](docs/ui.md).

## Project docs
- Contributor guide: [`CONTRIBUTING.md`](CONTRIBUTING.md)
- Security policy: [`SECURITY.md`](SECURITY.md)
- Code ownership: [`.github/CODEOWNERS`](.github/CODEOWNERS)
- Environment template: [`.env.example`](.env.example)
- Standards: [`docs/standards.md`](docs/standards.md)
- Module boundaries: [`docs/module-boundaries.md`](docs/module-boundaries.md)
- Public API surface: [`docs/public-api.md`](docs/public-api.md)
- Deprecation policy: [`docs/deprecation-policy.md`](docs/deprecation-policy.md)
- Operability primitives: [`docs/operability.md`](docs/operability.md)
- Incident runbooks index: [`docs/runbooks/README.md`](docs/runbooks/README.md)
- Runbook: destination offline: [`docs/runbooks/destination-offline.md`](docs/runbooks/destination-offline.md)
- Runbook: verify failure: [`docs/runbooks/verify-failure.md`](docs/runbooks/verify-failure.md)
- Runbook: corruption suspicion: [`docs/runbooks/corruption-suspicion.md`](docs/runbooks/corruption-suspicion.md)
- Runbook: stuck daemon: [`docs/runbooks/stuck-daemon.md`](docs/runbooks/stuck-daemon.md)
- Threat model notes: [`docs/threat-model.md`](docs/threat-model.md)
- Security runbook: [`docs/security-runbook.md`](docs/security-runbook.md)
- Architecture narrative: [`docs/architecture.md`](docs/architecture.md)
- ADR index: [`docs/adr/README.md`](docs/adr/README.md)
- Design principles: [`docs/design-principles.md`](docs/design-principles.md)
- Risk register: [`docs/risk-register.md`](docs/risk-register.md)
- Delivery roadmap: [`docs/roadmap.md`](docs/roadmap.md)
- Branch protection policy: [`docs/branch-protection.md`](docs/branch-protection.md)
- Release process: [`docs/release.md`](docs/release.md)
- Release notes template: [`docs/templates/release-notes.md`](docs/templates/release-notes.md)
- Performance tuning guide: [`docs/performance-tuning.md`](docs/performance-tuning.md)
- Storage architecture: [`docs/storage.md`](docs/storage.md)
- UI architecture: [`docs/ui.md`](docs/ui.md)

## Setup
1. Install Rust stable, Node 18 or newer, and npm.
2. Enable git hooks with `make hooks`.
3. On macOS, install the optional native desktop bridge helper with `./scripts/install_desktopctl.sh` when you need the desktop tooling in `tools/desktopctl`.
4. Build everything with `make build`.
5. Run the desktop app with `make run`.

Quickstart (dev):
- `./start` (builds prerequisites and launches the desktop app)

Local-only paths:
- `.env`, `.env.*` except `.env.example`, `.reports/`, `.npm-cache/`, `.vscode/`, `.codex/`, `projects/`, and frontend coverage output are intentionally ignored so local state and secrets do not enter Git.

Public-repo contract:
- Git must contain everything required to rebuild the app from a clean clone except local secrets and machine-specific runtime state.
- Pre-commit, pre-push, and CI all run a repository hygiene gate that blocks tracked local state, machine-specific absolute paths, restore-breaking omissions, and oversized tracked files.
- To restore after local disk loss, clone the repo, install toolchains, run `make frontend-install`, then `make build` or `make run`.

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
- Generate encryption key: `cargo run -p cli -- keygen`
- One off backup: `cargo run -p cli -- run-once` with `--dry-run`
- Verify integrity: `cargo run -p cli -- verify`
- Diagnostics: `cargo run -p cli -- doctor`
- Install service on login: `cargo run -p cli -- install-service --enable`

## Configuration

Config file path:
- `~/.config/backup_sync/config.toml`

Environment layering for startup config:
- `BACKUP_SYNC_CONFIG` overrides the config file path.
- `BACKUP_SYNC_INTERVAL_SECONDS` overrides `interval_seconds`.
- `BACKUP_SYNC_SAFE_MODE` overrides `safe_mode` (`true/false/1/0/yes/no/on/off`).

State file path:
- `~/.config/backup_sync/state.json`

UI tuning knobs live in `crates/gui/frontend/src/config/uiTuning.ts`.
Backend config defaults and guardrails are centralized in `crates/core/src/config/registry.rs`.
Security notes:
- Run the daemon as your user account and avoid elevated privileges unless required by your platform.

Example config:
```toml
backup_root = "/Users/me/Backups"
interval_seconds = 1800
max_backups_per_file = 5
ignore_patterns = ["**/node_modules/**", "**/target/**", "**/.DS_Store"]
safe_mode = false

[encryption]
enabled = false
# key_id = "..." # generated by `cargo run -p cli -- keygen`
# key_path = "..." # optional override; defaults to ~/.config/backup_sync/key_v1.bin on most platforms
# blob_chunk_bytes = 65536

[compression]
enabled = false
# blob_chunk_bytes = 65536
# zstd_level = 3

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
force_full_scan_interval_cycles = 24
verify_interval_seconds = 86400
watcher_debounce_seconds = 2
ipc_timeout_seconds = 5
service_command_timeout_seconds = 15
service_command_retry_delay_ms = 300
service_command_poll_interval_ms = 50
gui_start_hidden = false
tray_tooltip_refresh_seconds = 10
log_tail_lines = 200
simulation_sample_limit = 10
replication_enabled = true
replication_mirror_manifests = true
replication_max_manifest_deletes_per_cycle = 500

[[destinations]]
id = "primary"
path = "/Users/me/Backups"
label = "Primary"
replicate_to = ["mirror"]

[[destinations]]
id = "mirror"
path = "/Volumes/BackupMirror/Backups"
label = "Mirror"

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
- IPC critical-flow load checks run via `make perf-ipc-load` and are included in `make perf-check`.
- Resource tuning guidance is documented in [`docs/performance-tuning.md`](docs/performance-tuning.md).

## Testing
- Backend unit and integration tests: `cargo test -p backup_core -p daemon -p cli`
- Frontend unit tests: `npm --prefix crates/gui/frontend test`
- End to end smoke test: `cargo test -p backup_core --test e2e_smoke`
- Format and lint: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, `npm --prefix crates/gui/frontend run lint`
- Operability policy enforcement: `make operability-check`
- Lockfile hygiene check: `make lockfile-check`
- Cargo deny policy check: `make deny`
- Security gate bundle: `make security-check`
- Secret scanning (working tree + full history): `make secrets`
- Perf guard: `make perf-check`
- Full quality gate run: `make ci` (format, lint, typecheck, tests with coverage, perf guard, security audits)

## Troubleshooting
- If the UI cannot connect, ensure the daemon is running and the IPC socket is reachable.
- If backups are skipped, verify watched paths exist and are outside the destination.
- If verification reports issues, run `backup-sync verify` and review recent logs.
- For detailed diagnostics, export a doctor report from the UI or CLI.
- If the dev server port conflicts with another program, set `BACKUP_SYNC_DEV_PORT` or let Vite pick the repo-specific default in `crates/gui/frontend/vite.config.ts`.
- Incident playbooks:
  - destination offline: `docs/runbooks/destination-offline.md`
  - verify failure: `docs/runbooks/verify-failure.md`
  - corruption suspicion: `docs/runbooks/corruption-suspicion.md`
  - stuck daemon: `docs/runbooks/stuck-daemon.md`

## Release notes
- Changes are tracked in `CHANGELOG.md`.
- Versioning and tagging guidance lives in [`docs/release.md`](docs/release.md).

## Notable code paths
This system emphasizes bounded IO, explicit config validation, and performance guardrails that stay visible in code review.
- `crates/core/src/backup/versioned/store.rs` implements the SHA-256 blob store, manifests, retention, and garbage collection.
- `crates/core/src/backup/versioned/restore.rs` reconstructs versions with atomic writes and optional in-place cleanup.
- `crates/core/src/hashing.rs` centralizes SHA-256 hashing and hex formatting used across subsystems.
- `crates/core/src/config/registry.rs` centralizes defaults and validation limits for config knobs.
- `crates/core/src/bin/perf_guard.rs` captures and validates performance baselines.
- `crates/daemon/src/runtime/loop.rs` coordinates lifecycle, IPC, and graceful shutdown.
- `crates/gui/frontend/src/config/uiTuning.ts` consolidates UI tuning knobs.
- `crates/gui/frontend/src/components/minimal/MinimalMain.tsx` is the spec-minimal main screen UI.
