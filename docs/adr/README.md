# Architecture Decision Records (ADRs)

Last updated: 2026-02-20

This directory records high-impact architecture decisions. ADRs are immutable once accepted; superseding decisions must reference the prior ADR explicitly.

## Status legend
- Proposed: under active review
- Accepted: approved and in use
- Superseded: replaced by a newer ADR

## ADR index
1. [0001: Versioned content-addressed backup engine](0001-versioned-content-addressed-engine.md) - Accepted
2. [0002: Clean layer boundaries with `core` as dependency root](0002-core-as-dependency-root.md) - Accepted
3. [0003: Local IPC contract for daemon health and control](0003-local-ipc-contract.md) - Accepted
4. [0004: Centralized config registry with fail-fast validation](0004-central-config-registry.md) - Accepted
5. [0005: Unified boundary error model and structured logging](0005-unified-error-model-and-structured-logging.md) - Accepted
6. [0006: Deterministic quality gates as merge contract](0006-deterministic-quality-gates.md) - Accepted

## ADR lifecycle
1. Open a PR with one ADR per major decision.
2. Include context, options considered, chosen decision, and consequences.
3. Link impacted code paths and migration plan when behavior changes.
4. Mark superseded ADRs instead of deleting them.
