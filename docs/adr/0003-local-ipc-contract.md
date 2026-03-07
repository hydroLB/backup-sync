# ADR 0003: Local IPC Contract for Daemon Health and Control

- Status: Accepted
- Date: 2026-02-20

## Context
The system has multiple operator entrypoints (CLI and GUI) that need consistent status, health, and command semantics against the daemon.

## Decision
Use a local IPC channel as the canonical daemon control plane, including explicit health/readiness request and response contracts.

## Consequences
- CLI and GUI share a single source of truth for daemon state.
- Correlation IDs can be propagated end-to-end for diagnostics.
- Health/readiness checks are transport-stable and testable.
- Future clients can be added without re-implementing runtime semantics.

## Alternatives considered
- Direct file-based polling of daemon state (rejected for weaker liveness guarantees).
- Separate ad hoc APIs per client (rejected due to contract drift risk).
