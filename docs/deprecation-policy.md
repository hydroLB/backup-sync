# Deprecation Policy for Public Contracts

Last updated: 2026-02-20

This policy defines how public contracts are deprecated and removed so upgrades stay predictable.

## Public contracts in scope
- Rust public exports documented in `docs/public-api.md`.
- CLI commands, flags, and output contracts in `crates/cli`.
- Daemon IPC request/response contracts in `crates/daemon/src/runtime/ipc.rs`.
- Config schema and semantic knobs in `crates/core/src/config`.

## Compatibility guarantees
1. No breaking contract change in PATCH releases.
2. No breaking contract change in MINOR releases unless behind an explicitly marked opt-in feature flag.
3. Contract removals and incompatible behavior changes occur only in MAJOR releases.

## Required deprecation lifecycle
1. Announce: mark the contract as deprecated in docs and release notes.
2. Warn: emit clear warning text at boundary call sites where feasible (CLI output, logs, docs).
3. Support window: keep deprecated behavior for at least one MINOR release or 90 days, whichever is longer.
4. Remove: remove only in a MAJOR release with migration notes.

## Required PR artifacts for deprecation
Every deprecation PR must include:
1. Contract inventory update in `docs/public-api.md` (or related contract doc).
2. `CHANGELOG.md` entry under `Unreleased` with a `Deprecated` section.
3. Migration instructions in PR description and release notes.
4. Tests that cover deprecated-path behavior until removal.

## Removal checklist
Before removal in a MAJOR release:
1. Confirm deprecation was previously announced and documented.
2. Confirm migration path exists and is tested.
3. Remove deprecated behavior and stale compatibility branches.
4. Update docs and release notes with explicit upgrade steps.

## Exceptions
Security fixes may shorten timelines if exploitation risk is high. Any exception must be documented in `CHANGELOG.md` and the release notes.
