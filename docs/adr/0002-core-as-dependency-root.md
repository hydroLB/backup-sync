# ADR 0002: Clean Layer Boundaries with `core` as Dependency Root

- Status: Accepted
- Date: 2026-02-20

## Context
As the project adds features across daemon, CLI, and GUI, duplicated business logic and bidirectional dependencies increase regression risk.

## Decision
Define `crates/core` as the dependency root for backup domain behavior. `daemon`, `cli`, and `gui` may depend on `core`; `core` does not depend on those crates.

## Consequences
- Domain rules and invariants are implemented once.
- Boundary crates remain focused on transport, orchestration, and UX.
- Layer violations can be checked automatically in CI.
- Refactors are less risky because business semantics are centralized.

## Alternatives considered
- Feature ownership split by process (rejected due to duplicated rules).
- Shared utility crate without explicit ownership boundaries (rejected as too vague).
