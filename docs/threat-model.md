# Threat Model Notes

## Scope
Focused on critical local attack surfaces in this repository:
1. Daemon IPC interface
2. Config loading and environment overrides
3. Path handling for watched sources and backup destinations

## Trust Boundaries
1. User/process boundary:
   - GUI, CLI, and daemon processes can exchange IPC payloads.
   - Inputs crossing process boundaries are untrusted.
2. Filesystem boundary:
   - Config, state, and data paths can be user-controlled or stale.
3. Environment boundary:
   - Startup env vars can override runtime config paths/values.

## Assets
1. Backup data integrity and availability.
2. Local filesystem confidentiality (avoid sensitive path/secret leakage).
3. Operational safety signals (health/readiness correctness).

## Threat Surfaces and Controls

### IPC Surface (`crates/daemon/src/runtime/ipc.rs`)
Threats:
1. Malformed or oversized IPC payloads causing resource exhaustion.
2. Log injection through untrusted request identifiers.
3. Stale or spoofed operational probe traffic confusing operators.

Controls:
1. Bounded request read size and chunked parsing using runtime limits.
2. Timeout around IPC read/write operations.
3. Correlation/request id sanitization before log emission.
4. Structured machine-parseable logs with stable fields.
5. Explicit status/health/readiness contracts with request-id echo.

Residual risk:
1. Local same-user process can still send high-rate valid requests; rate limiting is not yet enforced.

### Config Surface (`crates/core/src/config/*`)
Threats:
1. Invalid or hostile config values triggering unsafe runtime behavior.
2. Drift between defaults and validation allowing out-of-range knobs.
3. Environment overrides bypassing expected startup invariants.

Controls:
1. Single validated config load path for entrypoints.
2. Validation limits for timeout/retry/backoff and size bounds.
3. Fail-fast startup behavior on invalid config.

Residual risk:
1. Intentional misconfiguration by local user remains possible; guardrails reduce but do not eliminate operator error.

### Path Handling Surface (`crates/core/src/fs/*`, daemon cycle + GUI commands)
Threats:
1. Unsafe path traversal or invalid destination selection.
2. Sensitive absolute paths leaking into logs/errors.
3. Destination disconnect/reconnect races causing unsafe writes.

Controls:
1. Canonicalization and path validation before critical operations.
2. Redaction helpers for path/text logging.
3. Destination health checks and pause/resume behavior.
4. Readiness probe reflects safe-mode and destination pause/unavailable states.

Residual risk:
1. Symlink and mount churn between validation and use remains a local TOCTOU class risk.

## Detection and Response Hooks
1. CI security gates (`ci-security`): secrets, lockfile hygiene, deny policy, advisories.
2. Local runbook: `docs/security-runbook.md`.
3. Structured logs with correlation for IPC and shutdown operations.

## Priority Follow-ups
1. Add IPC request rate limiting/backpressure for local abuse resistance.
2. Add explicit symlink policy notes and tests for high-risk path operations.
3. Add periodic review cadence for advisory ignore exceptions in `deny.toml`.
