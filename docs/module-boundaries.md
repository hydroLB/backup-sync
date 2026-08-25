# Module Boundaries and API Ownership

## Purpose
This document defines dependency direction, package responsibilities, and where new code should live.

## Dependency Direction
Allowed direction for runtime dependencies:

1. `crates/core` is the domain and storage engine. It must not depend on app-specific UI/runtime layers.
2. `crates/daemon`, `crates/cli`, and `crates/gui` may depend on `crates/core` only (no direct cross-dependencies).
3. The desktop build of `crates/gui/frontend` talks to `crates/gui` IPC commands and should not duplicate Rust backup rules. The hosted build may select the isolated `src/runtime/web` adapter, whose scope is limited to browser-owned data and the existing frontend command contract.

Do not introduce reverse dependencies from `core` into `daemon`, `cli`, `gui`, or frontend.
Do not add direct `cli <-> daemon`, `gui <-> daemon`, or `cli <-> gui` imports/dependencies. Keep interactions at process or IPC boundaries.

Automated enforcement lives in `/scripts/check-boundaries.sh` and runs through `make boundaries-check` (included in `make check` and CI).

## Package Responsibilities

### `crates/core`
- Owns backup domain logic, content-addressed blob storage, retention, restore, config models, and validation.
- Public API surface is defined by exports from `crates/core/src/lib.rs`.
- Internal implementation details stay inside module trees under `src/`.

### `crates/daemon`
- Owns long-running runtime loop, scheduling, IPC server behavior, and service lifecycle integration.
- External behavior contract is daemon CLI/runtime entry behavior plus IPC protocol semantics.

### `crates/cli`
- Owns operational command UX (`status`, `run-once`, `verify`, `doctor`, service management).
- Should orchestrate `core` APIs and external process invocations, not duplicate domain logic.

### `crates/gui` and `crates/gui/src-tauri`
- Own Tauri command handlers, tray integration, and desktop process orchestration.
- Must keep command handlers thin and delegate business logic to `core`.

### `crates/gui/frontend`
- Owns React presentation, client-side interaction state, and IPC request wiring.
- Keeps native and hosted transports behind the same service boundary so the rendered application stays shared.
- May implement browser-only storage behavior under `src/runtime/web`; it must not be imported by Rust crates or represented as the native engine.
- Browser behavior that mirrors a native contract requires byte-level tests for hashing, versioning, integrity validation, and restore safety.

## Public API Ownership
Code ownership is enforced by `.github/CODEOWNERS`. Treat these surfaces as public contracts:

1. `crates/core/src/lib.rs` exported contracts (see `docs/public-api.md`)
2. `crates/daemon/src/lib.rs` exported contracts (see `docs/public-api.md`)
3. `crates/gui/src/lib.rs` exported contracts (see `docs/public-api.md`)
4. Daemon IPC request/response contract (`crates/daemon/src/runtime/ipc.rs`) including status, health, and readiness probes
5. CLI command contract (`crates/cli/src/commands/*`)
6. Frontend service contract wrappers (`crates/gui/frontend/src/services/*`)

Changes to these surfaces must include:

1. Contract-level tests
2. Backward compatibility note in PR description (or explicit breakage statement)
3. Matching docs update when behavior or usage changes

## Where to Put New Code
Use this placement guide for future PRs:

1. Backup logic, manifests, retention, restore, hashing, config validation:
   - Place in `crates/core/src/...`
2. Scheduling loop, IPC transport, daemon process behavior:
   - Place in `crates/daemon/src/...`
3. User-facing command flow or diagnostics:
   - Place in `crates/cli/src/commands/...`
4. Desktop command bridge, tray/menu behavior:
   - Place in `crates/gui/src/...`
5. UI rendering/state and IPC calls:
   - Place in `crates/gui/frontend/src/...`
6. Browser-local implementations of the frontend command contract:
   - Place only in `crates/gui/frontend/src/runtime/web/...`
7. Cross-cutting process docs and architecture notes:
   - Place in `docs/...`

If a change touches more than one layer, keep domain logic in the lowest valid layer and keep upper layers as thin adapters.
