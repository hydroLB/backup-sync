# Performance and Resource Tuning

Last updated: 2026-02-20

This guide defines the performance guardrails and resource knobs used to keep backup cycles predictable.

## Performance gates
1. Core hot-path regression guard:
   - `cargo run -p backup_core --bin perf_guard -- --baseline docs/perf-baseline.json --max-ratio 1.5`
2. IPC critical-flow load test:
   - `cargo test -p daemon --test ipc_load -- --nocapture`
3. Canonical combined gate:
   - `make perf-check`

`make perf-check` is wired into `make check` and CI.

## Microbenchmark coverage
The core benchmark suite (`crates/core/benches/core_hot_paths.rs`) currently tracks:
1. SHA-256 hashing throughput for 1 MiB, 16 MiB, and 64 MiB payloads.
2. Versioned simulate cycle latency for 200-file and 1000-file watched trees.

Run with:
- `cargo bench -p backup_core --features bench`

## IPC load-test contract
`crates/daemon/tests/ipc_load.rs` enforces:
1. 120 health requests over the daemon IPC channel.
2. bounded concurrency (`8` workers).
3. p95 and total-duration thresholds.

Threshold overrides:
- `BACKUP_SYNC_IPC_LOAD_MAX_P95_MS` (default `400`)
- `BACKUP_SYNC_IPC_LOAD_MAX_TOTAL_MS` (default `6000`)

## Resource knobs and guardrails

| Knob | Default | Validation max | Operational guidance |
|---|---:|---:|---|
| `max_parallel_copies` | `1` | `64` | Keep `1-4` for laptops; raise gradually on dedicated hosts. |
| `execution.copy_buffer_bytes` | `65536` | `8388608` | Increase for throughput, decrease if memory pressure is visible. |
| `hashing.buffer_bytes` | `65536` | `8388608` | Keep aligned with copy buffer for balanced IO. |
| `execution.copy_timeout_seconds` | `300` | `3600` | Raise only for slow network/external storage. |
| `runtime.ipc_timeout_seconds` | `5` | `60` | Keep low to preserve UI/CLI responsiveness. |
| `runtime.ipc_request_max_bytes` | `65536` | `16777216` | Increase only for larger control payload contracts. |
| `runtime.ipc_response_max_bytes` | `4194304` | `67108864` | Raise carefully; large responses can increase memory spikes. |
| `runtime.ipc_read_chunk_bytes` | `8192` | `1048576` | Raise for fewer syscalls, lower for tighter memory limits. |
| `runtime.daemon_shutdown_join_timeout_seconds` | `30` | `3600` | Keep near default unless cleanup operations require longer drain time. |
| `execution.retry_delays_ms` | `[100,200,400,800]` | max `10` entries, each <= `30000` | Keep non-decreasing backoff; avoid aggressive retry storms. |

## Capacity planning heuristics
1. Approximate in-process IO buffer footprint:
   - `max_parallel_copies * (execution.copy_buffer_bytes + hashing.buffer_bytes)`
2. For low-memory systems, tune down in this order:
   - `max_parallel_copies`
   - `execution.copy_buffer_bytes`
   - `hashing.buffer_bytes`
3. For high-throughput hosts, tune up in this order:
   - `max_parallel_copies`
   - `execution.copy_buffer_bytes`
   - optional `max_bytes_per_second` cap

## Recommended tuning profiles
1. Laptop-safe baseline
   - `max_parallel_copies = 1`
   - `execution.copy_buffer_bytes = 65536`
   - `hashing.buffer_bytes = 65536`
2. Balanced desktop
   - `max_parallel_copies = 2`
   - `execution.copy_buffer_bytes = 131072`
   - `hashing.buffer_bytes = 131072`
3. Dedicated backup host (verify thermals and IO first)
   - `max_parallel_copies = 4`
   - `execution.copy_buffer_bytes = 262144`
   - `hashing.buffer_bytes = 262144`
