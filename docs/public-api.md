# Public API Surface and Ownership

## Purpose
This document defines the intentionally exported Rust API surface for each crate. Anything not listed here is internal and can change without cross-crate compatibility guarantees.

## Policy
1. Internal modules are private by default (`mod`, not `pub mod`).
2. New public API must be exposed from the crate root (`src/lib.rs`) via explicit `pub use` or documented `pub mod`.
3. Any PR that changes public API must update this file and include tests for the updated contract.
4. Ownership and review routing are enforced by [`.github/CODEOWNERS`](../.github/CODEOWNERS).

## Crate Surfaces

### `backup_core` (`crates/core/src/lib.rs`)
Explicit modules intended for cross-crate consumption:
- `backup`
- `config`
- `encryption`
- `fs`
- `io`
- `logging`
- `platform`
- `service`
- `state`

Stable root re-exports intended as primary call sites:
- config load/save/validate entrypoints
- fail-fast startup load entrypoints (`load_validated_config`, `load_validated_from_path`)
- core model types (`Config`, `RuntimeTuning`, etc.)
- state/store types (`StoredState`, `StateStore`, `ActivityItem`, `SafetyWarning`)
- backup runtime entrypoints (`BackupExecutor`, `verify_backups`)
- hashing entrypoints (`sha256_file_hex`, `sha256_file_hex_with_tuning`, `sha256_hex`)

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

### `tauri-plugin-single-instance` (`crates/tauri-plugin-single-instance/src/lib.rs`)
Explicit public contract:
- `init`

## Public API Change Checklist
1. Update crate root exports in `src/lib.rs`.
2. Update this document in the same PR.
3. Update integration or contract tests that exercise external usage.
4. Call out API compatibility impact in the PR description.
