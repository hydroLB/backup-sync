# Operability Primitives

## Purpose
This document defines the production operability contract for daemon liveness/readiness checks, graceful shutdown behavior, and timeout/retry/backoff policy enforcement.

## Related Performance Docs
- Performance and resource tuning: [`docs/performance-tuning.md`](performance-tuning.md)

## Related Incident Playbooks
- Runbook index: [`docs/runbooks/README.md`](runbooks/README.md)
- Destination offline: [`docs/runbooks/destination-offline.md`](runbooks/destination-offline.md)
- Verify failure: [`docs/runbooks/verify-failure.md`](runbooks/verify-failure.md)
- Corruption suspicion: [`docs/runbooks/corruption-suspicion.md`](runbooks/corruption-suspicion.md)
- Stuck daemon: [`docs/runbooks/stuck-daemon.md`](runbooks/stuck-daemon.md)

## Daemon IPC Health and Readiness Contract
Daemon probes are served over the same local IPC channel as status requests (`backup_sync_ipc.sock` on Unix, `\\.\\pipe\\backup_sync_ipc` on Windows).

### Health probe
Request:
```json
{"type":"HealthWithContext","payload":{"request_id":"probe-123","source":"healthcheck"}}
```

Response fields:
- `request_id`: echoed/sanitized correlation id
- `healthy`: liveness boolean (`true` means daemon process and IPC loop are serving)
- `status`: machine string (`"healthy"`)
- `checked_at_ts`: unix timestamp
- `uptime_secs`: daemon uptime when available
- `version`: daemon version when available

### Readiness probe
Request:
```json
{"type":"ReadinessWithContext","payload":{"request_id":"probe-456","source":"readiness"}}
```

Response fields:
- `request_id`: echoed/sanitized correlation id
- `ready`: readiness boolean for backup writes
- `status`: machine string (`"ready"` or `"not_ready"`)
- `reason`: machine reason when not ready
- `checked_at_ts`: unix timestamp
- `safe_mode`: current safe-mode state
- `destination_paused`: destination pause state
- `destination_pause_reason`: pause reason when set
- `destination_unavailable_ids`: unavailable destination ids

Readiness is `false` when any of the following hold:
- `safe_mode == true`
- `destination_paused == true`
- `destination_unavailable_ids` is non-empty

## Graceful Shutdown Contract
Daemon shutdown coordination guarantees:
1. Broadcast shutdown signal to all runtime tasks.
2. Await IPC, scheduler, and verify tasks with `runtime.daemon_shutdown_join_timeout_seconds`.
3. Abort tasks that exceed the timeout to prevent hung process exit.
4. Persist final state snapshot before process exit.
5. Remove Unix IPC socket path during shutdown cleanup.

## Timeout, Retry, and Backoff Policy
All external/IO boundaries must use explicit bounded behavior with config-backed knobs.

| Surface | Policy | Knobs |
|---|---|---|
| Daemon IPC read/write | Every operation wrapped by timeout; bounded request/response size | `runtime.ipc_timeout_seconds`, `runtime.ipc_request_max_bytes`, `runtime.ipc_response_max_bytes`, `runtime.ipc_read_chunk_bytes` |
| Service command orchestration | Bounded execution + retry delay | `runtime.service_command_timeout_seconds`, `runtime.service_command_retry_delay_ms`, `runtime.service_command_poll_interval_ms` |
| Backup execution and blob writes | Retry with non-decreasing backoff + bounded copy/hash time | `execution.retry_delays_ms`, `execution.retry_jitter_pct`, `execution.copy_timeout_seconds`, `hashing.timeout_seconds` |
| Blocking IO loops | Explicit polling backoff interval bound | `execution.blocking_io_backoff_poll_interval_ms` |
| Shutdown join behavior | Graceful join with abort fallback | `runtime.daemon_shutdown_join_timeout_seconds` |

## Enforcement Points
Operability policy is enforced through:
1. Config validation guardrails in `crates/core/src/config/validate/sections/tuning.rs`.
2. Runtime timeout usage at daemon IPC and task-join boundaries.
3. Automated policy check script: `scripts/check-operability.sh`.
4. Local and CI gate integration via `make operability-check`.

## Test Coverage
- IPC contract tests: `crates/daemon/tests/ipc_status.rs`
- IPC load test: `crates/daemon/tests/ipc_load.rs`
- IPC request parsing and reply tests: `crates/daemon/src/runtime/ipc.rs` unit tests
- Graceful shutdown timeout/abort behavior tests: `crates/daemon/src/runtime/loop.rs` unit tests
