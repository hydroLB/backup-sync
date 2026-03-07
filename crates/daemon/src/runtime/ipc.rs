use anyhow::{Context, Result};
use backup_core::io::{run_with_policy, BlockingIoPolicy, CancellationFlag};
use backup_core::StoredState;
use chrono::Utc;
use fs2::free_space;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::{watch, Mutex};
use tokio::time::timeout;
use tracing::{error, info, warn};

use std::time::{Duration, Instant};
#[cfg(unix)]
use tokio::io::{AsyncReadExt, AsyncWriteExt};
#[cfg(windows)]
use tokio::io::{AsyncReadExt, AsyncWriteExt};
#[cfg(windows)]
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
#[cfg(unix)]
use tokio::net::UnixListener;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
enum Request {
    Status,
    StatusWithContext {
        request_id: String,
        source: String,
    },
    Health,
    HealthWithContext {
        request_id: String,
        source: String,
    },
    Readiness,
    ReadinessWithContext {
        request_id: String,
        source: String,
    },
    SetSafeMode {
        enabled: bool,
        #[serde(default)]
        request_id: Option<String>,
        #[serde(default)]
        source: Option<String>,
    },
    ClearSafetyWarning,
    ClearSafetyWarningWithContext {
        request_id: String,
        source: String,
    },
}

#[derive(Serialize, Deserialize, Clone)]
struct DestinationStatus {
    id: String,
    label: Option<String>,
    path: PathBuf,
    reachable: bool,
    writable: bool,
    free_bytes: Option<u64>,
    message: String,
}

#[derive(Serialize, Deserialize)]
struct StatusReply {
    request_id: String,
    last_run_ts: Option<i64>,
    last_files_backed_up: usize,
    last_error: Option<String>,
    last_dirty_count: usize,
    last_safety_warning: Option<backup_core::SafetyWarning>,
    uptime_secs: Option<i64>,
    version: Option<String>,
    free_bytes: Option<u64>,
    last_verify_ts: Option<i64>,
    last_verify_status: Option<String>,
    last_verify_issues: Option<usize>,
    recent_activity: Vec<backup_core::ActivityItem>,
    safe_mode: bool,
    destination_paused: bool,
    destination_pause_reason: Option<String>,
    destination_unavailable_ids: Vec<String>,
    destination_last_unavailable_ts: Option<i64>,
    destination_last_recovered_ts: Option<i64>,
    replication_last_run_ts: Option<i64>,
    replication_last_status: Option<String>,
    replication_last_error: Option<String>,
    replication_last_bytes_copied: u64,
    replication_last_blobs_copied: usize,
    replication_last_manifests_copied: usize,
    replication_last_manifests_deleted: usize,
    replication_last_pairs_ok: usize,
    replication_last_pairs_failed: usize,
    replication_last_targets_failed: Vec<String>,
    destinations: Vec<DestinationStatus>,
}

#[derive(Serialize, Deserialize)]
struct HealthReply {
    request_id: String,
    healthy: bool,
    status: String,
    checked_at_ts: i64,
    uptime_secs: Option<i64>,
    version: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct ReadinessReply {
    request_id: String,
    ready: bool,
    status: String,
    reason: Option<String>,
    checked_at_ts: i64,
    safe_mode: bool,
    destination_paused: bool,
    destination_pause_reason: Option<String>,
    destination_unavailable_ids: Vec<String>,
}

#[derive(Serialize)]
struct AckReply {
    request_id: String,
    ok: bool,
}

#[derive(Clone, Debug)]
struct RequestContext {
    request_id: String,
    source: String,
    operation: &'static str,
}

const METRIC_IPC_REQUEST_TOTAL: &str = "daemon_ipc_request_total";
const METRIC_IPC_REQUEST_FAILURE_TOTAL: &str = "daemon_ipc_request_failure_total";
const METRIC_IPC_REQUEST_LATENCY: &str = "daemon_ipc_request_latency_ms";

/// Summary: Reads free space for a path, logging warnings when it fails.
///
/// Inputs: the path to check.
///
/// Outputs: an optional free space value.
///
/// Side effects: Reads filesystem free space and emits warnings on failure.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: IPC status payload construction.
///
/// Why this exists: report free space when available without failing the IPC response.
fn free_space_or_warn(path: &Path) -> Option<u64> {
    match free_space(path) {
        Ok(bytes) => Some(bytes),
        Err(e) => {
            warn!(
                component = "daemon",
                subsystem = "ipc",
                action = "free_space_probe_failed",
                path = %backup_core::logging::redact_path(path),
                error = %e,
                "daemon IPC free-space probe failed"
            );
            None
        }
    }
}

/// Summary: Builds normalized request context from a parsed IPC request.
///
/// Inputs: parsed request value.
///
/// Outputs: request context containing operation name, request id, and source.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: IPC logging and response correlation.
///
/// Why this exists: keep request-level observability metadata consistent across IPC operations.
fn request_context(request: &Request) -> RequestContext {
    match request {
        Request::Status => RequestContext {
            request_id: backup_core::logging::correlation_id("daemon-ipc", None),
            source: "unknown".to_string(),
            operation: "status",
        },
        Request::StatusWithContext { request_id, source } => RequestContext {
            request_id: backup_core::logging::correlation_id("daemon-ipc", Some(request_id)),
            source: backup_core::logging::sanitize_correlation_id(source),
            operation: "status",
        },
        Request::Health => RequestContext {
            request_id: backup_core::logging::correlation_id("daemon-ipc", None),
            source: "unknown".to_string(),
            operation: "health",
        },
        Request::HealthWithContext { request_id, source } => RequestContext {
            request_id: backup_core::logging::correlation_id("daemon-ipc", Some(request_id)),
            source: backup_core::logging::sanitize_correlation_id(source),
            operation: "health",
        },
        Request::Readiness => RequestContext {
            request_id: backup_core::logging::correlation_id("daemon-ipc", None),
            source: "unknown".to_string(),
            operation: "readiness",
        },
        Request::ReadinessWithContext { request_id, source } => RequestContext {
            request_id: backup_core::logging::correlation_id("daemon-ipc", Some(request_id)),
            source: backup_core::logging::sanitize_correlation_id(source),
            operation: "readiness",
        },
        Request::SetSafeMode {
            request_id, source, ..
        } => RequestContext {
            request_id: backup_core::logging::correlation_id("daemon-ipc", request_id.as_deref()),
            source: source
                .as_deref()
                .map(backup_core::logging::sanitize_correlation_id)
                .unwrap_or_else(|| "unknown".to_string()),
            operation: "set_safe_mode",
        },
        Request::ClearSafetyWarning => RequestContext {
            request_id: backup_core::logging::correlation_id("daemon-ipc", None),
            source: "unknown".to_string(),
            operation: "clear_safety_warning",
        },
        Request::ClearSafetyWarningWithContext { request_id, source } => RequestContext {
            request_id: backup_core::logging::correlation_id("daemon-ipc", Some(request_id)),
            source: backup_core::logging::sanitize_correlation_id(source),
            operation: "clear_safety_warning",
        },
    }
}

/// Summary: Builds an IPC status reply from stored state and destination metadata.
///
/// Inputs: the stored state, destination list, and activity limit.
///
/// Outputs: a `StatusReply` ready for serialization.
///
/// Side effects: Reads filesystem free space metadata for destinations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: IPC request handling for status calls.
///
/// Why this exists: centralize status payload construction for the UI and CLI.
fn build_status_reply(
    state: &StoredState,
    destinations: &[backup_core::config::model::Destination],
    recent_activity_limit: usize,
    request_id: String,
) -> StatusReply {
    let uptime_secs = state.start_ts.map(|ts| Utc::now().timestamp() - ts);
    let dest_status: Vec<_> = destinations
        .iter()
        .map(|d| {
            let reachable = d.path.exists() && d.path.is_dir();
            let free_bytes = if reachable {
                free_space_or_warn(&d.path)
            } else {
                None
            };
            let writable = free_bytes.is_some();
            let message = if !reachable {
                "Destination not found (drive disconnected?)".to_string()
            } else if writable {
                "OK".to_string()
            } else {
                "Destination present but free space unavailable".to_string()
            };
            DestinationStatus {
                id: d.id.clone(),
                label: d.label.clone(),
                path: d.path.clone(),
                reachable,
                writable,
                free_bytes,
                message,
            }
        })
        .collect();
    let free_bytes = dest_status.first().and_then(|d| d.free_bytes);
    StatusReply {
        request_id,
        last_run_ts: state.last_run_ts,
        last_files_backed_up: state.last_files_backed_up,
        last_error: state.last_error.clone(),
        last_dirty_count: state.last_dirty_count,
        last_safety_warning: state.last_safety_warning.clone(),
        uptime_secs,
        version: state.version.clone(),
        free_bytes,
        last_verify_ts: state.last_verify_ts,
        last_verify_status: state.last_verify_status.clone(),
        last_verify_issues: state.last_verify_issues,
        recent_activity: state
            .recent_activity
            .iter()
            .rev()
            .take(recent_activity_limit)
            .cloned()
            .collect(),
        safe_mode: state.safe_mode,
        destination_paused: state.destination_paused,
        destination_pause_reason: state.destination_pause_reason.clone(),
        destination_unavailable_ids: state.destination_unavailable_ids.clone(),
        destination_last_unavailable_ts: state.destination_last_unavailable_ts,
        destination_last_recovered_ts: state.destination_last_recovered_ts,
        replication_last_run_ts: state.replication_last_run_ts,
        replication_last_status: state.replication_last_status.clone(),
        replication_last_error: state.replication_last_error.clone(),
        replication_last_bytes_copied: state.replication_last_bytes_copied,
        replication_last_blobs_copied: state.replication_last_blobs_copied,
        replication_last_manifests_copied: state.replication_last_manifests_copied,
        replication_last_manifests_deleted: state.replication_last_manifests_deleted,
        replication_last_pairs_ok: state.replication_last_pairs_ok,
        replication_last_pairs_failed: state.replication_last_pairs_failed,
        replication_last_targets_failed: state.replication_last_targets_failed.clone(),
        destinations: dest_status,
    }
}

/// Summary: Builds a liveness health payload for daemon operability probes.
///
/// Inputs: current stored state snapshot and request id.
///
/// Outputs: a `HealthReply` for IPC clients.
///
/// Side effects: Reads current timestamp.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon IPC request handling for health probes.
///
/// Why this exists: expose a stable machine-checkable liveness contract over daemon IPC.
fn build_health_reply(state: &StoredState, request_id: String) -> HealthReply {
    HealthReply {
        request_id,
        healthy: true,
        status: "healthy".to_string(),
        checked_at_ts: Utc::now().timestamp(),
        uptime_secs: state.start_ts.map(|ts| Utc::now().timestamp() - ts),
        version: state.version.clone(),
    }
}

/// Summary: Builds a readiness payload indicating whether writes can proceed safely.
///
/// Inputs: current stored state snapshot and request id.
///
/// Outputs: a `ReadinessReply` for IPC clients.
///
/// Side effects: Reads current timestamp.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon IPC request handling for readiness probes.
///
/// Why this exists: provide a deterministic readiness contract for operability checks.
fn build_readiness_reply(state: &StoredState, request_id: String) -> ReadinessReply {
    let reason = if state.safe_mode {
        Some("safe_mode_enabled".to_string())
    } else if state.destination_paused {
        state
            .destination_pause_reason
            .clone()
            .or_else(|| Some("destination_paused".to_string()))
    } else if !state.destination_unavailable_ids.is_empty() {
        Some("destinations_unavailable".to_string())
    } else {
        None
    };
    let ready = reason.is_none();
    ReadinessReply {
        request_id,
        ready,
        status: if ready {
            "ready".to_string()
        } else {
            "not_ready".to_string()
        },
        reason,
        checked_at_ts: Utc::now().timestamp(),
        safe_mode: state.safe_mode,
        destination_paused: state.destination_paused,
        destination_pause_reason: state.destination_pause_reason.clone(),
        destination_unavailable_ids: state.destination_unavailable_ids.clone(),
    }
}

/// Summary: Writes a serialized status reply to the IPC stream and closes it.
///
/// Inputs: a writable stream and the status reply.
///
/// Outputs: `Ok(())` when the reply is written and the stream is closed.
///
/// Side effects: Writes to the IPC stream and shuts it down.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: IPC status request handling.
///
/// Why this exists: ensure IPC responses complete cleanly for clients.
async fn write_status_reply<W>(writer: &mut W, reply: &StatusReply) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let payload = serde_json::to_string(reply)
        .context("daemon::runtime::ipc write_status_reply failed to serialize reply")?;
    writer
        .write_all(payload.as_bytes())
        .await
        .context("daemon::runtime::ipc write_status_reply failed to write status reply")?;
    if let Err(error) = writer.shutdown().await {
        warn!(
            component = "daemon",
            subsystem = "ipc",
            action = "status_reply_shutdown_failed",
            error = %error,
            "daemon IPC status reply shutdown failed"
        );
    }
    Ok(())
}

/// Summary: Writes a serialized health reply to the IPC stream and closes it.
///
/// Inputs: writable stream and health reply payload.
///
/// Outputs: `Ok(())` when write and shutdown complete.
///
/// Side effects: Writes to the IPC stream and shuts it down.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon IPC health request handling.
///
/// Why this exists: keep health response writes explicit and testable.
async fn write_health_reply<W>(writer: &mut W, reply: &HealthReply) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let payload = serde_json::to_string(reply)
        .context("daemon::runtime::ipc write_health_reply failed to serialize reply")?;
    writer
        .write_all(payload.as_bytes())
        .await
        .context("daemon::runtime::ipc write_health_reply failed to write reply")?;
    if let Err(error) = writer.shutdown().await {
        warn!(
            component = "daemon",
            subsystem = "ipc",
            action = "health_reply_shutdown_failed",
            error = %error,
            "daemon IPC health reply shutdown failed"
        );
    }
    Ok(())
}

/// Summary: Writes a serialized readiness reply to the IPC stream and closes it.
///
/// Inputs: writable stream and readiness reply payload.
///
/// Outputs: `Ok(())` when write and shutdown complete.
///
/// Side effects: Writes to the IPC stream and shuts it down.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon IPC readiness request handling.
///
/// Why this exists: keep readiness response writes explicit and testable.
async fn write_readiness_reply<W>(writer: &mut W, reply: &ReadinessReply) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let payload = serde_json::to_string(reply)
        .context("daemon::runtime::ipc write_readiness_reply failed to serialize reply")?;
    writer
        .write_all(payload.as_bytes())
        .await
        .context("daemon::runtime::ipc write_readiness_reply failed to write reply")?;
    if let Err(error) = writer.shutdown().await {
        warn!(
            component = "daemon",
            subsystem = "ipc",
            action = "readiness_reply_shutdown_failed",
            error = %error,
            "daemon IPC readiness reply shutdown failed"
        );
    }
    Ok(())
}

async fn write_ack_reply<W>(writer: &mut W, ok: bool, request_id: &str) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let payload = serde_json::to_string(&AckReply {
        ok,
        request_id: request_id.to_string(),
    })
    .context("daemon::runtime::ipc write_ack_reply failed to serialize reply")?;
    writer
        .write_all(payload.as_bytes())
        .await
        .context("daemon::runtime::ipc write_ack_reply failed to write reply")?;
    if let Err(error) = writer.shutdown().await {
        warn!(
            component = "daemon",
            subsystem = "ipc",
            action = "ack_reply_shutdown_failed",
            request_id = %request_id,
            error = %error,
            "daemon IPC ack reply shutdown failed"
        );
    }
    Ok(())
}

/// Summary: Reads and parses a JSON IPC request without requiring the client to close the stream.
///
/// Inputs: a readable IPC stream, a timeout bound, and a max request size.
///
/// Outputs: a parsed `Request` value.
///
/// Side effects: Reads bytes from the IPC stream until a request is parsed or limits are exceeded.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: both Unix socket and Windows named pipe IPC servers.
///
/// Why this exists: Avoid deadlocks where both sides wait for EOF; allow small request/response exchanges.
async fn read_request<R>(
    reader: &mut R,
    ipc_timeout: Duration,
    max_request_bytes: usize,
    read_chunk_bytes: usize,
) -> Result<Request>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = vec![0u8; read_chunk_bytes.max(1)];
    loop {
        if buf.len() > max_request_bytes {
            anyhow::bail!(
                "daemon::runtime::ipc read_request exceeded max request size {} bytes",
                max_request_bytes
            );
        }
        let n = match timeout(ipc_timeout, reader.read(&mut chunk)).await {
            Ok(Ok(n)) => n,
            Ok(Err(e)) => {
                return Err(
                    anyhow::anyhow!(e).context("daemon::runtime::ipc read_request read error")
                );
            }
            Err(_) => {
                anyhow::bail!(
                    "daemon::runtime::ipc read_request timed out after {:?}",
                    ipc_timeout
                );
            }
        };
        if n == 0 {
            if buf.is_empty() {
                anyhow::bail!("daemon::runtime::ipc read_request got empty request");
            }
            // EOF after some bytes; attempt a final parse below.
        } else {
            buf.extend_from_slice(&chunk[..n]);
        }

        match serde_json::from_slice::<Request>(&buf) {
            Ok(req) => return Ok(req),
            Err(e) => {
                if e.is_eof() && n != 0 {
                    continue;
                }
                return Err(anyhow::anyhow!(e)
                    .context("daemon::runtime::ipc read_request failed to parse request"));
            }
        }
    }
}

#[cfg(unix)]
/// Summary: Spawns a Unix socket based IPC server for status requests.
///
/// Inputs: shared state, destination list, IPC timeout, activity limit, and shutdown receiver.
///
/// Outputs: a join handle for the IPC task.
///
/// Side effects: Binds a Unix socket and performs IPC reads and writes.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon IPC handling for GUI and CLI clients.
///
/// Why this exists: expose status over a local IPC channel.
pub async fn spawn_server(
    state: Arc<Mutex<StoredState>>,
    destinations: Vec<backup_core::config::model::Destination>,
    ipc_timeout: Duration,
    max_request_bytes: usize,
    read_chunk_bytes: usize,
    recent_activity_limit: usize,
    shutdown: watch::Receiver<bool>,
) -> Result<tokio::task::JoinHandle<()>> {
    let socket_path = socket_path()?;
    let io_policy = BlockingIoPolicy::bootstrap_defaults();
    if socket_path.exists() {
        if let Err(e) = run_with_policy(
            "daemon::runtime::ipc::spawn_server remove stale socket",
            &io_policy,
            CancellationFlag::none(),
            || {
                std::fs::remove_file(&socket_path).map_err(|error| {
                    anyhow::anyhow!(error)
                        .context("daemon::runtime::ipc::spawn_server failed remove stale socket")
                })
            },
        ) {
            warn!(
                component = "daemon",
                subsystem = "ipc",
                action = "remove_stale_socket_failed",
                socket_path = %backup_core::logging::redact_path(&socket_path),
                error = %backup_core::logging::redact_text(&format!("{e:#}")),
                "daemon IPC failed removing stale socket"
            );
        }
    }
    let listener = UnixListener::bind(&socket_path).with_context(|| {
        format!(
            "daemon::runtime::ipc spawn_server failed to bind IPC socket at {:?}",
            socket_path
        )
    })?;
    info!(
        component = "daemon",
        subsystem = "ipc",
        action = "listener_bound",
        socket_path = %backup_core::logging::redact_path(&socket_path),
        "daemon IPC listener bound"
    );
    Ok(tokio::spawn(async move {
        let mut shutdown = shutdown;
        loop {
            if *shutdown.borrow() {
                info!(
                    component = "daemon",
                    subsystem = "ipc",
                    action = "shutdown_requested",
                    "daemon IPC shutdown requested"
                );
                break;
            }
            let accept_result = tokio::select! {
                changed = shutdown.changed() => {
                    match changed {
                        Ok(()) => {
                            if *shutdown.borrow() {
                                info!(
                                    component = "daemon",
                                    subsystem = "ipc",
                                    action = "shutdown_signal_observed",
                                    "daemon IPC shutdown signal observed"
                                );
                                break;
                            }
                            continue;
                        }
                        Err(_) => {
                            info!(
                                component = "daemon",
                                subsystem = "ipc",
                                action = "shutdown_channel_closed",
                                "daemon IPC shutdown channel closed"
                            );
                            break;
                        }
                    }
                }
                accepted = listener.accept() => accepted,
            };
            match accept_result {
                Ok((mut stream, _addr)) => {
                    let request_started = Instant::now();
                    let req = match read_request(
                        &mut stream,
                        ipc_timeout,
                        max_request_bytes,
                        read_chunk_bytes,
                    )
                    .await
                    {
                        Ok(r) => r,
                        Err(e) => {
                            backup_core::metrics::counter_inc(METRIC_IPC_REQUEST_FAILURE_TOTAL, 1);
                            error!(
                                component = "daemon",
                                subsystem = "ipc",
                                action = "request_read_failed",
                                error = %backup_core::logging::redact_text(&format!("{e:#}")),
                                "daemon IPC request read failed"
                            );
                            continue;
                        }
                    };
                    let context = request_context(&req);
                    backup_core::metrics::counter_inc(METRIC_IPC_REQUEST_TOTAL, 1);
                    info!(
                        component = "daemon",
                        subsystem = "ipc",
                        action = "request_received",
                        request_id = %context.request_id,
                        source = %context.source,
                        operation = %context.operation,
                        "daemon IPC request received"
                    );
                    match req {
                        Request::Status | Request::StatusWithContext { .. } => {
                            let st = state.lock().await.clone();
                            let reply = build_status_reply(
                                &st,
                                &destinations,
                                recent_activity_limit,
                                context.request_id.clone(),
                            );
                            match timeout(ipc_timeout, write_status_reply(&mut stream, &reply))
                                .await
                            {
                                Ok(Ok(_)) => {}
                                Ok(Err(e)) => {
                                    backup_core::metrics::counter_inc(
                                        METRIC_IPC_REQUEST_FAILURE_TOTAL,
                                        1,
                                    );
                                    error!(
                                        component = "daemon",
                                        subsystem = "ipc",
                                        action = "request_write_failed",
                                        request_id = %context.request_id,
                                        source = %context.source,
                                        operation = %context.operation,
                                        error = %backup_core::logging::redact_text(&format!("{e:#}")),
                                        "daemon IPC response write failed"
                                    );
                                }
                                Err(_) => {
                                    backup_core::metrics::counter_inc(
                                        METRIC_IPC_REQUEST_FAILURE_TOTAL,
                                        1,
                                    );
                                    error!(
                                        component = "daemon",
                                        subsystem = "ipc",
                                        action = "request_write_timeout",
                                        request_id = %context.request_id,
                                        source = %context.source,
                                        operation = %context.operation,
                                        timeout_ms = ipc_timeout.as_millis() as u64,
                                        "daemon IPC response write timed out"
                                    );
                                }
                            }
                        }
                        Request::Health | Request::HealthWithContext { .. } => {
                            let st = state.lock().await.clone();
                            let reply = build_health_reply(&st, context.request_id.clone());
                            match timeout(ipc_timeout, write_health_reply(&mut stream, &reply))
                                .await
                            {
                                Ok(Ok(_)) => {}
                                Ok(Err(e)) => {
                                    backup_core::metrics::counter_inc(
                                        METRIC_IPC_REQUEST_FAILURE_TOTAL,
                                        1,
                                    );
                                    error!(
                                        component = "daemon",
                                        subsystem = "ipc",
                                        action = "request_write_failed",
                                        request_id = %context.request_id,
                                        source = %context.source,
                                        operation = %context.operation,
                                        error = %backup_core::logging::redact_text(&format!("{e:#}")),
                                        "daemon IPC response write failed"
                                    );
                                }
                                Err(_) => {
                                    backup_core::metrics::counter_inc(
                                        METRIC_IPC_REQUEST_FAILURE_TOTAL,
                                        1,
                                    );
                                    error!(
                                        component = "daemon",
                                        subsystem = "ipc",
                                        action = "request_write_timeout",
                                        request_id = %context.request_id,
                                        source = %context.source,
                                        operation = %context.operation,
                                        timeout_ms = ipc_timeout.as_millis() as u64,
                                        "daemon IPC response write timed out"
                                    );
                                }
                            }
                        }
                        Request::Readiness | Request::ReadinessWithContext { .. } => {
                            let st = state.lock().await.clone();
                            let reply = build_readiness_reply(&st, context.request_id.clone());
                            match timeout(ipc_timeout, write_readiness_reply(&mut stream, &reply))
                                .await
                            {
                                Ok(Ok(_)) => {}
                                Ok(Err(e)) => {
                                    backup_core::metrics::counter_inc(
                                        METRIC_IPC_REQUEST_FAILURE_TOTAL,
                                        1,
                                    );
                                    error!(
                                        component = "daemon",
                                        subsystem = "ipc",
                                        action = "request_write_failed",
                                        request_id = %context.request_id,
                                        source = %context.source,
                                        operation = %context.operation,
                                        error = %backup_core::logging::redact_text(&format!("{e:#}")),
                                        "daemon IPC response write failed"
                                    );
                                }
                                Err(_) => {
                                    backup_core::metrics::counter_inc(
                                        METRIC_IPC_REQUEST_FAILURE_TOTAL,
                                        1,
                                    );
                                    error!(
                                        component = "daemon",
                                        subsystem = "ipc",
                                        action = "request_write_timeout",
                                        request_id = %context.request_id,
                                        source = %context.source,
                                        operation = %context.operation,
                                        timeout_ms = ipc_timeout.as_millis() as u64,
                                        "daemon IPC response write timed out"
                                    );
                                }
                            }
                        }
                        Request::SetSafeMode { enabled, .. } => {
                            {
                                let mut st = state.lock().await;
                                st.safe_mode = enabled;
                            }
                            match timeout(
                                ipc_timeout,
                                write_ack_reply(&mut stream, true, &context.request_id),
                            )
                            .await
                            {
                                Ok(Ok(_)) => {}
                                Ok(Err(e)) => {
                                    backup_core::metrics::counter_inc(
                                        METRIC_IPC_REQUEST_FAILURE_TOTAL,
                                        1,
                                    );
                                    error!(
                                        component = "daemon",
                                        subsystem = "ipc",
                                        action = "request_write_failed",
                                        request_id = %context.request_id,
                                        source = %context.source,
                                        operation = %context.operation,
                                        error = %backup_core::logging::redact_text(&format!("{e:#}")),
                                        "daemon IPC response write failed"
                                    );
                                }
                                Err(_) => {
                                    backup_core::metrics::counter_inc(
                                        METRIC_IPC_REQUEST_FAILURE_TOTAL,
                                        1,
                                    );
                                    error!(
                                        component = "daemon",
                                        subsystem = "ipc",
                                        action = "request_write_timeout",
                                        request_id = %context.request_id,
                                        source = %context.source,
                                        operation = %context.operation,
                                        timeout_ms = ipc_timeout.as_millis() as u64,
                                        "daemon IPC response write timed out"
                                    );
                                }
                            }
                        }
                        Request::ClearSafetyWarning
                        | Request::ClearSafetyWarningWithContext { .. } => {
                            {
                                let mut st = state.lock().await;
                                st.last_safety_warning = None;
                            }
                            match timeout(
                                ipc_timeout,
                                write_ack_reply(&mut stream, true, &context.request_id),
                            )
                            .await
                            {
                                Ok(Ok(_)) => {}
                                Ok(Err(e)) => {
                                    backup_core::metrics::counter_inc(
                                        METRIC_IPC_REQUEST_FAILURE_TOTAL,
                                        1,
                                    );
                                    error!(
                                        component = "daemon",
                                        subsystem = "ipc",
                                        action = "request_write_failed",
                                        request_id = %context.request_id,
                                        source = %context.source,
                                        operation = %context.operation,
                                        error = %backup_core::logging::redact_text(&format!("{e:#}")),
                                        "daemon IPC response write failed"
                                    );
                                }
                                Err(_) => {
                                    backup_core::metrics::counter_inc(
                                        METRIC_IPC_REQUEST_FAILURE_TOTAL,
                                        1,
                                    );
                                    error!(
                                        component = "daemon",
                                        subsystem = "ipc",
                                        action = "request_write_timeout",
                                        request_id = %context.request_id,
                                        source = %context.source,
                                        operation = %context.operation,
                                        timeout_ms = ipc_timeout.as_millis() as u64,
                                        "daemon IPC response write timed out"
                                    );
                                }
                            }
                        }
                    }
                    backup_core::metrics::observe_duration(
                        METRIC_IPC_REQUEST_LATENCY,
                        request_started.elapsed(),
                    );
                }
                Err(e) => {
                    if *shutdown.borrow() {
                        info!(
                            component = "daemon",
                            subsystem = "ipc",
                            action = "accept_ended_during_shutdown",
                            error = %backup_core::logging::redact_text(&format!("{e:#}")),
                            "daemon IPC accept ended during shutdown"
                        );
                    } else {
                        error!(
                            component = "daemon",
                            subsystem = "ipc",
                            action = "accept_failed",
                            error = %backup_core::logging::redact_text(&format!("{e:#}")),
                            "daemon IPC accept failed"
                        );
                    }
                    break;
                }
            }
        }
        if let Err(error) = run_with_policy(
            "daemon::runtime::ipc::spawn_server cleanup socket path",
            &BlockingIoPolicy::bootstrap_defaults(),
            CancellationFlag::none(),
            || {
                std::fs::remove_file(&socket_path).map_err(|io_error| {
                    anyhow::anyhow!(io_error).context(
                        "daemon::runtime::ipc::spawn_server failed removing socket on shutdown",
                    )
                })
            },
        ) {
            if error
                .downcast_ref::<std::io::Error>()
                .is_none_or(|io_error| io_error.kind() != std::io::ErrorKind::NotFound)
            {
                warn!(
                    component = "daemon",
                    subsystem = "ipc",
                    action = "cleanup_socket_failed",
                    socket_path = %backup_core::logging::redact_path(&socket_path),
                    error = %backup_core::logging::redact_text(&format!("{error:#}")),
                    "daemon IPC socket cleanup failed"
                );
            }
        }
    }))
}

#[cfg(windows)]
/// Summary: Spawns a Windows named pipe IPC server for status requests.
///
/// Inputs: shared state, destination list, IPC timeout, activity limit, and shutdown receiver.
///
/// Outputs: a join handle for the IPC task.
///
/// Side effects: Creates a named pipe and performs IPC reads and writes.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon IPC handling for GUI and CLI clients.
///
/// Why this exists: expose status over a local IPC channel on Windows.
pub async fn spawn_server(
    _state: Arc<Mutex<StoredState>>,
    destinations: Vec<backup_core::config::model::Destination>,
    ipc_timeout: Duration,
    max_request_bytes: usize,
    read_chunk_bytes: usize,
    recent_activity_limit: usize,
    shutdown: watch::Receiver<bool>,
) -> Result<tokio::task::JoinHandle<()>> {
    let pipe_name = r"\\.\pipe\backup_sync_ipc";
    let state = _state;
    Ok(tokio::spawn(async move {
        let mut shutdown = shutdown;
        loop {
            if *shutdown.borrow() {
                info!(
                    component = "daemon",
                    subsystem = "ipc",
                    action = "shutdown_requested",
                    "daemon IPC shutdown requested"
                );
                break;
            }
            match ServerOptions::new()
                .first_pipe_instance(true)
                .create(pipe_name)
            {
                Ok(mut server) => {
                    let request_started = Instant::now();
                    let req_result = tokio::select! {
                        changed = shutdown.changed() => {
                            match changed {
                                Ok(()) => {
                                    if *shutdown.borrow() {
                                        info!(
                                            component = "daemon",
                                            subsystem = "ipc",
                                            action = "shutdown_signal_observed",
                                            "daemon IPC shutdown signal observed"
                                        );
                                        break;
                                    }
                                    continue;
                                }
                                Err(_) => {
                                    info!(
                                        component = "daemon",
                                        subsystem = "ipc",
                                        action = "shutdown_channel_closed",
                                        "daemon IPC shutdown channel closed"
                                    );
                                    break;
                                }
                            }
                        }
                        req = read_request(&mut server, ipc_timeout, max_request_bytes, read_chunk_bytes) => req,
                    };
                    let req = match req_result {
                        Ok(r) => r,
                        Err(e) => {
                            backup_core::metrics::counter_inc(METRIC_IPC_REQUEST_FAILURE_TOTAL, 1);
                            error!(
                                component = "daemon",
                                subsystem = "ipc",
                                action = "request_read_failed",
                                error = %backup_core::logging::redact_text(&format!("{e:#}")),
                                "daemon IPC request read failed"
                            );
                            continue;
                        }
                    };
                    let context = request_context(&req);
                    backup_core::metrics::counter_inc(METRIC_IPC_REQUEST_TOTAL, 1);
                    info!(
                        component = "daemon",
                        subsystem = "ipc",
                        action = "request_received",
                        request_id = %context.request_id,
                        source = %context.source,
                        operation = %context.operation,
                        "daemon IPC request received"
                    );
                    match req {
                        Request::Status | Request::StatusWithContext { .. } => {
                            let st = state.lock().await.clone();
                            let reply = build_status_reply(
                                &st,
                                &destinations,
                                recent_activity_limit,
                                context.request_id.clone(),
                            );
                            match timeout(ipc_timeout, write_status_reply(&mut server, &reply))
                                .await
                            {
                                Ok(Ok(_)) => {}
                                Ok(Err(e)) => {
                                    backup_core::metrics::counter_inc(
                                        METRIC_IPC_REQUEST_FAILURE_TOTAL,
                                        1,
                                    );
                                    error!(
                                        component = "daemon",
                                        subsystem = "ipc",
                                        action = "request_write_failed",
                                        request_id = %context.request_id,
                                        source = %context.source,
                                        operation = %context.operation,
                                        error = %backup_core::logging::redact_text(&format!("{e:#}")),
                                        "daemon IPC response write failed"
                                    );
                                }
                                Err(_) => {
                                    backup_core::metrics::counter_inc(
                                        METRIC_IPC_REQUEST_FAILURE_TOTAL,
                                        1,
                                    );
                                    error!(
                                        component = "daemon",
                                        subsystem = "ipc",
                                        action = "request_write_timeout",
                                        request_id = %context.request_id,
                                        source = %context.source,
                                        operation = %context.operation,
                                        timeout_ms = ipc_timeout.as_millis() as u64,
                                        "daemon IPC response write timed out"
                                    );
                                }
                            }
                        }
                        Request::Health | Request::HealthWithContext { .. } => {
                            let st = state.lock().await.clone();
                            let reply = build_health_reply(&st, context.request_id.clone());
                            match timeout(ipc_timeout, write_health_reply(&mut server, &reply))
                                .await
                            {
                                Ok(Ok(_)) => {}
                                Ok(Err(e)) => {
                                    backup_core::metrics::counter_inc(
                                        METRIC_IPC_REQUEST_FAILURE_TOTAL,
                                        1,
                                    );
                                    error!(
                                        component = "daemon",
                                        subsystem = "ipc",
                                        action = "request_write_failed",
                                        request_id = %context.request_id,
                                        source = %context.source,
                                        operation = %context.operation,
                                        error = %backup_core::logging::redact_text(&format!("{e:#}")),
                                        "daemon IPC response write failed"
                                    );
                                }
                                Err(_) => {
                                    backup_core::metrics::counter_inc(
                                        METRIC_IPC_REQUEST_FAILURE_TOTAL,
                                        1,
                                    );
                                    error!(
                                        component = "daemon",
                                        subsystem = "ipc",
                                        action = "request_write_timeout",
                                        request_id = %context.request_id,
                                        source = %context.source,
                                        operation = %context.operation,
                                        timeout_ms = ipc_timeout.as_millis() as u64,
                                        "daemon IPC response write timed out"
                                    );
                                }
                            }
                        }
                        Request::Readiness | Request::ReadinessWithContext { .. } => {
                            let st = state.lock().await.clone();
                            let reply = build_readiness_reply(&st, context.request_id.clone());
                            match timeout(ipc_timeout, write_readiness_reply(&mut server, &reply))
                                .await
                            {
                                Ok(Ok(_)) => {}
                                Ok(Err(e)) => {
                                    backup_core::metrics::counter_inc(
                                        METRIC_IPC_REQUEST_FAILURE_TOTAL,
                                        1,
                                    );
                                    error!(
                                        component = "daemon",
                                        subsystem = "ipc",
                                        action = "request_write_failed",
                                        request_id = %context.request_id,
                                        source = %context.source,
                                        operation = %context.operation,
                                        error = %backup_core::logging::redact_text(&format!("{e:#}")),
                                        "daemon IPC response write failed"
                                    );
                                }
                                Err(_) => {
                                    backup_core::metrics::counter_inc(
                                        METRIC_IPC_REQUEST_FAILURE_TOTAL,
                                        1,
                                    );
                                    error!(
                                        component = "daemon",
                                        subsystem = "ipc",
                                        action = "request_write_timeout",
                                        request_id = %context.request_id,
                                        source = %context.source,
                                        operation = %context.operation,
                                        timeout_ms = ipc_timeout.as_millis() as u64,
                                        "daemon IPC response write timed out"
                                    );
                                }
                            }
                        }
                        Request::SetSafeMode { enabled, .. } => {
                            {
                                let mut st = state.lock().await;
                                st.safe_mode = enabled;
                            }
                            match timeout(
                                ipc_timeout,
                                write_ack_reply(&mut server, true, &context.request_id),
                            )
                            .await
                            {
                                Ok(Ok(_)) => {}
                                Ok(Err(e)) => {
                                    backup_core::metrics::counter_inc(
                                        METRIC_IPC_REQUEST_FAILURE_TOTAL,
                                        1,
                                    );
                                    error!(
                                        component = "daemon",
                                        subsystem = "ipc",
                                        action = "request_write_failed",
                                        request_id = %context.request_id,
                                        source = %context.source,
                                        operation = %context.operation,
                                        error = %backup_core::logging::redact_text(&format!("{e:#}")),
                                        "daemon IPC response write failed"
                                    );
                                }
                                Err(_) => {
                                    backup_core::metrics::counter_inc(
                                        METRIC_IPC_REQUEST_FAILURE_TOTAL,
                                        1,
                                    );
                                    error!(
                                        component = "daemon",
                                        subsystem = "ipc",
                                        action = "request_write_timeout",
                                        request_id = %context.request_id,
                                        source = %context.source,
                                        operation = %context.operation,
                                        timeout_ms = ipc_timeout.as_millis() as u64,
                                        "daemon IPC response write timed out"
                                    );
                                }
                            }
                        }
                        Request::ClearSafetyWarning
                        | Request::ClearSafetyWarningWithContext { .. } => {
                            {
                                let mut st = state.lock().await;
                                st.last_safety_warning = None;
                            }
                            match timeout(
                                ipc_timeout,
                                write_ack_reply(&mut server, true, &context.request_id),
                            )
                            .await
                            {
                                Ok(Ok(_)) => {}
                                Ok(Err(e)) => {
                                    backup_core::metrics::counter_inc(
                                        METRIC_IPC_REQUEST_FAILURE_TOTAL,
                                        1,
                                    );
                                    error!(
                                        component = "daemon",
                                        subsystem = "ipc",
                                        action = "request_write_failed",
                                        request_id = %context.request_id,
                                        source = %context.source,
                                        operation = %context.operation,
                                        error = %backup_core::logging::redact_text(&format!("{e:#}")),
                                        "daemon IPC response write failed"
                                    );
                                }
                                Err(_) => {
                                    backup_core::metrics::counter_inc(
                                        METRIC_IPC_REQUEST_FAILURE_TOTAL,
                                        1,
                                    );
                                    error!(
                                        component = "daemon",
                                        subsystem = "ipc",
                                        action = "request_write_timeout",
                                        request_id = %context.request_id,
                                        source = %context.source,
                                        operation = %context.operation,
                                        timeout_ms = ipc_timeout.as_millis() as u64,
                                        "daemon IPC response write timed out"
                                    );
                                }
                            }
                        }
                    }
                    backup_core::metrics::observe_duration(
                        METRIC_IPC_REQUEST_LATENCY,
                        request_started.elapsed(),
                    );
                }
                Err(e) => {
                    if *shutdown.borrow() {
                        info!(
                            component = "daemon",
                            subsystem = "ipc",
                            action = "pipe_accept_ended_during_shutdown",
                            error = %backup_core::logging::redact_text(&format!("{e:#}")),
                            "daemon IPC pipe accept ended during shutdown"
                        );
                    } else {
                        error!(
                            component = "daemon",
                            subsystem = "ipc",
                            action = "pipe_accept_failed",
                            error = %backup_core::logging::redact_text(&format!("{e:#}")),
                            "daemon IPC pipe accept failed"
                        );
                    }
                    break;
                }
            }
        }
    }))
}

/// Summary: Builds the Unix socket path used for IPC.
///
/// Inputs: none.
///
/// Outputs: the filesystem path for the IPC socket.
///
/// Side effects: Reads the runtime directory or temp directory.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: IPC server setup on Unix platforms.
///
/// Why this exists: centralize the IPC socket location in one helper.
pub fn socket_path() -> Result<PathBuf> {
    let mut p = dirs::runtime_dir().unwrap_or(std::env::temp_dir());
    p.push("backup_sync_ipc.sock");
    Ok(p)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Summary: read_request_parses_without_client_shutdown orchestrates this method's core behavior.
    ///
    /// Inputs: Method parameters and required receiver state.
    ///
    /// Outputs: Return value and observable result for callers.
    ///
    /// Side effects: None beyond this method's explicit operations.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: Invoked by and composes with adjacent module methods.
    ///
    /// Why this exists: Keeps this behavior isolated, testable, and reusable.
    #[tokio::test]
    async fn read_request_parses_without_client_shutdown() {
        let (mut client, mut server) = tokio::net::UnixStream::pair().unwrap();
        let runtime = backup_core::config::model::RuntimeTuning::default();
        let server_task = tokio::spawn(async move {
            let req = read_request(
                &mut server,
                Duration::from_secs(1),
                runtime.ipc_request_max_bytes,
                runtime.ipc_read_chunk_bytes,
            )
            .await
            .unwrap();
            assert!(matches!(req, Request::Status));
        });
        client.write_all(br#"{"type":"Status"}"#).await.unwrap();
        // Intentionally do not shutdown the client; server must still parse.
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn read_request_parses_contextual_status_request() {
        let (mut client, mut server) = tokio::net::UnixStream::pair().unwrap();
        let runtime = backup_core::config::model::RuntimeTuning::default();
        let server_task = tokio::spawn(async move {
            let req = read_request(
                &mut server,
                Duration::from_secs(1),
                runtime.ipc_request_max_bytes,
                runtime.ipc_read_chunk_bytes,
            )
            .await
            .unwrap();
            match req {
                Request::StatusWithContext { request_id, source } => {
                    assert_eq!(request_id, "req-123");
                    assert_eq!(source, "gui");
                }
                _ => panic!("expected Request::StatusWithContext"),
            }
        });
        client
            .write_all(br#"{"type":"StatusWithContext","payload":{"request_id":"req-123","source":"gui"}}"#)
            .await
            .unwrap();
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn read_request_parses_contextual_health_request() {
        let (mut client, mut server) = tokio::net::UnixStream::pair().unwrap();
        let runtime = backup_core::config::model::RuntimeTuning::default();
        let server_task = tokio::spawn(async move {
            let req = read_request(
                &mut server,
                Duration::from_secs(1),
                runtime.ipc_request_max_bytes,
                runtime.ipc_read_chunk_bytes,
            )
            .await
            .unwrap();
            match req {
                Request::HealthWithContext { request_id, source } => {
                    assert_eq!(request_id, "health-123");
                    assert_eq!(source, "probe");
                }
                _ => panic!("expected Request::HealthWithContext"),
            }
        });
        client
            .write_all(
                br#"{"type":"HealthWithContext","payload":{"request_id":"health-123","source":"probe"}}"#,
            )
            .await
            .unwrap();
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn write_ack_reply_includes_request_id() {
        let (mut client, mut server) = tokio::io::duplex(256);
        let server_task = tokio::spawn(async move {
            write_ack_reply(&mut server, true, "cid-42").await.unwrap();
        });
        let mut payload = Vec::new();
        client.read_to_end(&mut payload).await.unwrap();
        server_task.await.unwrap();
        let ack: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(ack["ok"], true);
        assert_eq!(ack["request_id"], "cid-42");
    }

    #[test]
    fn build_readiness_reply_reports_not_ready_when_safe_mode_enabled() {
        let state = StoredState {
            safe_mode: true,
            ..StoredState::default()
        };
        let reply = build_readiness_reply(&state, "req-ready-1".to_string());
        assert!(!reply.ready);
        assert_eq!(reply.status, "not_ready");
        assert_eq!(reply.reason.as_deref(), Some("safe_mode_enabled"));
        assert_eq!(reply.request_id, "req-ready-1");
    }
}
