# Public API Surface and Ownership

## Purpose
This document defines the intentionally exported Rust API surface for each crate. Anything not listed here is internal and can change without cross-crate compatibility guarantees.

## Policy
1. Internal modules are private by default (`mod`, not `pub mod`).
2. New public API must be exposed from the crate root (`src/lib.rs`) via explicit `pub use` or documented `pub mod`.
3. Any PR that changes public API must update this file and include tests for the updated contract.
4. Ownership and review routing are defined by [`.github/CODEOWNERS`](../.github/CODEOWNERS) and enforced only when GitHub branch protection or rulesets require code-owner review.

## Crate Surfaces

### `backup_core` (`crates/core/src/lib.rs`)
Explicit modules intended for cross-crate consumption:
- `backup`
- `boundary_error`
- `config`
- `encryption`
- `fs`
- `io`
- `logging`
- `metrics`
- `platform`
- `service`
- `state`

Stable root re-exports intended as primary call sites:
- canonical boundary error classification (`classify_anyhow`, `CanonicalErrorCode`, `ClassifiedBoundaryError`)
- config load/save/validate entrypoints
- fail-fast startup load entrypoints (`load_validated_config`, `load_validated_from_path`)
- core model types (`Config`, `RuntimeTuning`, etc.)
- state/store types (`StoredState`, `StateStore`, `ActivityItem`, `SafetyWarning`)
- state verification entrypoint (`verify_backups`)
- hashing entrypoints (`sha256_file_hex`, `sha256_file_hex_with_tuning`, `sha256_hex`)
- legacy compatibility re-exports for the original scan/plan/copy engine (`BackupExecutor`, `BackupResult`, `BackupPlan`, `PlannedItem`)

Legacy compatibility note:
- `BackupExecutor`, `BackupResult`, `BackupPlan`, and `PlannedItem` remain public for compatibility and internal test coverage.
- They are not the canonical production engine surface for new integrations.
- The daemon, CLI, and GUI use `backup::versioned` flows for run, verify, restore, and replication behavior.

Private-by-default top-level internals:
- `hashing` module internals (reachable only through root re-exports)
- `scheduling` module internals (reachable through `IntervalScheduler` re-export)

### `daemon` (`crates/daemon/src/lib.rs`)
Explicit modules intended for cross-crate consumption:
- `init`
- `runtime`

`init` public contract:
- `DaemonArgs`
- `EnvironmentSnapshot`
- `validate_environment`
- `ensure_parent_dir`
- `init_logging`

`runtime` public contract:
- `run_daemon`
- `spawn_server`
- `socket_path`
- `cid`

Daemon runtime IPC protocol contract (owned in `crates/daemon/src/runtime/ipc.rs`):
- status request/reply
- health request/reply
- readiness request/reply

### `gui` (`crates/gui/src/lib.rs`)
Explicit public contract:
- `run`
- `observability::cid`

All command wiring, tray internals, and API adapters are crate-private implementation details.

The optional single-instance behavior is supplied by the external `tauri-plugin-single-instance`
dependency. It is not a workspace crate or a Backup Sync public API surface.

## Public API Change Checklist
1. Update crate root exports in `src/lib.rs`.
2. Update this document in the same PR.
3. Update integration or contract tests that exercise external usage.
4. Call out API compatibility impact in the PR description.
