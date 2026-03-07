# Design Principles

Last updated: 2026-02-20

These principles define how Backup Sync is designed and how future changes are evaluated in code review.

## 1) Correctness before convenience
- Backup and restore behavior must be unambiguous and test-backed.
- Data integrity checks are first-class, not optional extras.
- Behavior changes require tests before implementation changes.

## 2) Domain logic in one place
- Core backup rules live in `crates/core`.
- CLI, daemon, and GUI are adapters that orchestrate, not duplicate domain behavior.
- New features should extend domain services first, then expose them at boundaries.

## 3) Bounded IO everywhere
- Every external call is bounded with explicit timeout and retry policy.
- Retries must be deterministic, observable, and safe for repeated execution.
- Long-running loops require cancellation and graceful shutdown paths.

## 4) Determinism as a quality gate
- `make check` is the canonical local and CI contract.
- Toolchains, dependencies, and scripts are pinned and reproducible.
- Tests must be stable across repeated local and CI runs.

## 5) Explicit contracts over implicit behavior
- Config is centralized, validated, and fail-fast.
- Public APIs are intentional, documented, and owned.
- Error outputs are structured and actionable at each boundary.

## 6) Operability is part of the product
- Structured logs and correlation IDs are required for multi-process flows.
- Health/readiness checks and runbooks must exist for critical incident classes.
- Recovery steps include rollback guidance, not only forward fixes.

## 7) Security by default
- Least privilege and secret-safe logging are baseline requirements.
- Supply-chain controls, dependency hygiene, and scanning stay enabled by default.
- Threat modeling decisions are documented and kept current as interfaces evolve.

## 8) Incremental change over rewrites
- Prefer small, reviewable PRs with behavior preservation.
- Remove dead code and reduce accidental complexity continuously.
- Introduce new abstractions only when they reduce long-term maintenance cost.

## Review checklist
Use this quick checklist before merge:
1. Does this change preserve or improve correctness with tests?
2. Does it keep domain ownership and dependency direction clean?
3. Are timeouts/retries/logging/error paths explicit and consistent?
4. Does `make check` pass locally without environment-specific workarounds?
5. Is the change easy to roll back if it misbehaves in production?
