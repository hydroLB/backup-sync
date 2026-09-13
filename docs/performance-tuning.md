# Performance and Resource Tuning

Last updated: 2026-09-13

This guide defines the performance guardrails and resource knobs used to keep backup cycles predictable.

## Performance gates
1. Core hot-path regression guard:
   - `cargo run -p backup_core --bin perf_guard -- --baseline docs/perf-baseline.json --max-ratio 1.5`
2. IPC critical-flow load test:
   - `cargo test -p daemon --test ipc_load -- --nocapture`
3. Canonical combined gate:
   - `make perf-check`

`make perf-check` is wired into `make check` and CI.

## Incremental workload baseline

The incremental check includes the complete default backup cycle for 200 files of 4 KiB each, after one file changes. This includes publishing the readable `Latest` recovery folder introduced with the current backup experience. The previous 38 ms baseline predates that output and does not represent the same work.

The updated incremental baseline is 121 ms, the median of three consecutive unoptimized Apple silicon/macOS measurements (121, 126, and 121 ms). The existing 1.5× regression limit, retry count, hashing baselines, and simulation baselines remain unchanged. Filesystem and host load affect these measurements; collect comparable runs and investigate workload changes before updating a baseline.

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
