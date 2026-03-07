use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

#[cfg(unix)]
mod support;

#[cfg(unix)]
const IPC_TIMEOUT: Duration = Duration::from_secs(3);
#[cfg(unix)]
const IPC_LOAD_REQUESTS: usize = 120;
#[cfg(unix)]
const IPC_LOAD_CONCURRENCY: usize = 8;

#[cfg(unix)]
/// Summary: Parses a u64 env var override for IPC load thresholds.
///
/// Inputs: env var name and default fallback.
///
/// Outputs: parsed value when valid, otherwise the fallback.
///
/// Side effects: reads process environment variables.
///
/// Error handling: ignores invalid env values and falls back to defaults.
///
/// Ties to other methods: threshold checks in `unix_ipc_health_load_contract`.
///
/// Why this exists: allow CI and local environments to tune strictness without code edits.
fn env_u64(name: &str, default_value: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default_value)
}

#[cfg(unix)]
/// Summary: Computes a percentile from a millisecond latency sample.
///
/// Inputs: mutable latency vector and percentile value in `[0, 100]`.
///
/// Outputs: selected percentile latency value.
///
/// Side effects: sorts the provided latency vector in place.
///
/// Error handling: returns zero when sample is empty.
///
/// Ties to other methods: p95 checks in `unix_ipc_health_load_contract`.
///
/// Why this exists: keep percentile computation deterministic and dependency free.
fn percentile_ms(samples: &mut [u64], percentile: u64) -> u64 {
    if samples.is_empty() {
        return 0;
    }
    samples.sort_unstable();
    let clamped = percentile.min(100) as usize;
    let index = ((samples.len() * clamped).div_ceil(100)).saturating_sub(1);
    samples[index]
}

#[cfg(unix)]
/// Summary: Executes one health request and captures request latency.
///
/// Inputs: IPC socket path and request index for correlation id generation.
///
/// Outputs: elapsed latency in milliseconds.
///
/// Side effects: opens a Unix socket client and performs one request/response exchange.
///
/// Error handling: returns contextual errors for transport or contract failures.
///
/// Ties to other methods: IPC helper in `support::send_json_request`.
///
/// Why this exists: isolate per-request work unit for concurrent load execution.
async fn run_health_request(path: std::path::PathBuf, idx: usize) -> Result<u64> {
    let request_id = format!("load-health-{idx:04}");
    let started = Instant::now();
    let payload = serde_json::json!({
        "type":"HealthWithContext",
        "payload":{"request_id":request_id,"source":"ipc-load-test"}
    });
    let reply = support::send_json_request(&path, &payload, IPC_TIMEOUT)
        .await
        .with_context(|| format!("ipc_load::run_health_request request {idx} failed"))?;
    if reply["healthy"] != true {
        anyhow::bail!("ipc_load::run_health_request {idx} expected healthy=true");
    }
    Ok(started.elapsed().as_millis() as u64)
}

#[cfg(unix)]
#[tokio::test]
/// Summary: Validates IPC health flow latency under bounded concurrent load.
///
/// Inputs: concurrent health requests over the daemon Unix socket.
///
/// Outputs: assertions on success count, p95 latency, and total wall-clock duration.
///
/// Side effects: starts daemon IPC server, performs repeated IPC calls, and records timing.
///
/// Error handling: skips on bind conflicts and fails on contract/latency regressions.
///
/// Ties to other methods: daemon health request handling and perf gate integration.
///
/// Why this exists: provide a lightweight load test that catches IPC critical-flow regressions.
async fn unix_ipc_health_load_contract() {
    let _ipc_lock = support::ipc_test_lock().await;
    support::cleanup_socket_file();

    let Some(server) = support::spawn_test_server("unix_ipc_health_load_contract").await else {
        return;
    };
    let path = server.socket_path().to_path_buf();

    let max_p95_ms = env_u64("BACKUP_SYNC_IPC_LOAD_MAX_P95_MS", 400);
    let max_total_ms = env_u64("BACKUP_SYNC_IPC_LOAD_MAX_TOTAL_MS", 6000);
    let start_all = Instant::now();
    let limiter = Arc::new(Semaphore::new(IPC_LOAD_CONCURRENCY));
    let mut join_set = tokio::task::JoinSet::new();

    for idx in 0..IPC_LOAD_REQUESTS {
        let permit =
            limiter.clone().acquire_owned().await.expect(
                "ipc_load::unix_ipc_health_load_contract failed to acquire semaphore permit",
            );
        let path_clone = path.clone();
        join_set.spawn(async move {
            let _permit = permit;
            run_health_request(path_clone, idx).await
        });
    }

    let mut latencies_ms = Vec::with_capacity(IPC_LOAD_REQUESTS);
    while let Some(joined) = join_set.join_next().await {
        let latency = joined
            .expect("ipc_load::unix_ipc_health_load_contract task join failed")
            .expect("ipc_load::unix_ipc_health_load_contract request failed");
        latencies_ms.push(latency);
    }
    let total_ms = start_all.elapsed().as_millis() as u64;
    let p95_ms = percentile_ms(&mut latencies_ms, 95);

    assert_eq!(
        latencies_ms.len(),
        IPC_LOAD_REQUESTS,
        "ipc_load::unix_ipc_health_load_contract expected all requests to complete"
    );
    assert!(
        p95_ms <= max_p95_ms,
        "ipc_load::unix_ipc_health_load_contract p95 latency {}ms exceeded {}ms (set BACKUP_SYNC_IPC_LOAD_MAX_P95_MS to tune)",
        p95_ms,
        max_p95_ms
    );
    assert!(
        total_ms <= max_total_ms,
        "ipc_load::unix_ipc_health_load_contract total duration {}ms exceeded {}ms (set BACKUP_SYNC_IPC_LOAD_MAX_TOTAL_MS to tune)",
        total_ms,
        max_total_ms
    );

    server.shutdown(IPC_TIMEOUT).await;
    support::cleanup_socket_file();
}
