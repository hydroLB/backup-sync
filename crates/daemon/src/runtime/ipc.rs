use anyhow::{Context, Result};
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
use tokio::net::windows::named_pipe::ServerOptions;
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt};

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

/// Report free space when available without failing the IPC response.
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

/// Keep request-level observability metadata consistent across IPC operations.
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

/// Centralize status payload construction for the UI and CLI.
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

/// Expose a stable machine-checkable liveness contract over daemon IPC.
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

/// Provide a deterministic readiness contract for operability checks.
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

/// Ensure IPC responses complete cleanly for clients.
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

/// Keep health response writes explicit and testable.
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

/// Keep readiness response writes explicit and testable.
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

/// Avoid deadlocks where both sides wait for EOF; allow small request/response exchanges.
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
            if buf.len() > max_request_bytes {
                anyhow::bail!(
                    "daemon::runtime::ipc read_request exceeded max request size {} bytes",
                    max_request_bytes
                );
            }
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EndpointIdentity {
    device: u64,
    inode: u64,
    owner: u32,
}

#[cfg(unix)]
fn effective_user_id() -> u32 {
    unsafe extern "C" {
        fn geteuid() -> u32;
    }

    // SAFETY: `geteuid` has no arguments, cannot mutate Rust memory, and is
    // available on every target covered by `cfg(unix)`.
    unsafe { geteuid() }
}

#[cfg(unix)]
fn endpoint_identity(path: &Path) -> Result<Option<EndpointIdentity>> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(anyhow::anyhow!(error).context(format!(
                "daemon::runtime::ipc failed to inspect endpoint at {path:?}"
            )));
        }
    };
    if !metadata.file_type().is_socket() {
        anyhow::bail!(
            "daemon IPC endpoint conflict at {path:?}: existing path is not a Unix socket; refusing to remove it"
        );
    }
    let owner = metadata.uid();
    let expected_owner = effective_user_id();
    if owner != expected_owner {
        anyhow::bail!(
            "daemon IPC endpoint conflict at {path:?}: socket is owned by uid {owner}, not current uid {expected_owner}; refusing to remove it"
        );
    }
    Ok(Some(EndpointIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
        owner,
    }))
}

#[cfg(unix)]
fn prepare_endpoint_directory(path: &Path) -> Result<()> {
    let directory = path
        .parent()
        .with_context(|| format!("daemon IPC endpoint path has no parent directory: {path:?}"))?;
    let injected = std::env::var_os("BACKUP_SYNC_IPC_SOCKET").is_some();
    if !directory.exists() && injected {
        anyhow::bail!(
            "daemon IPC injected endpoint parent directory does not exist: {directory:?}"
        );
    }
    if !injected {
        match std::fs::DirBuilder::new().mode(0o700).create(directory) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(anyhow::anyhow!(error).context(format!(
                    "daemon IPC failed to create private runtime directory {directory:?}"
                )));
            }
        }
    }
    let metadata = std::fs::symlink_metadata(directory)
        .with_context(|| format!("daemon IPC failed to inspect runtime directory {directory:?}"))?;
    if !metadata.file_type().is_dir() {
        anyhow::bail!("daemon IPC runtime path conflict at {directory:?}: expected a directory");
    }
    let expected_owner = effective_user_id();
    if metadata.uid() != expected_owner {
        anyhow::bail!(
            "daemon IPC runtime directory {directory:?} is not owned by current uid {expected_owner}"
        );
    }
    if !injected {
        std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700)).with_context(
            || format!("daemon IPC failed to secure runtime directory {directory:?}"),
        )?;
    } else if metadata.permissions().mode() & 0o077 != 0 {
        anyhow::bail!(
            "daemon IPC injected endpoint directory {directory:?} must not be accessible by group or other users"
        );
    }
    Ok(())
}

#[cfg(unix)]
fn remove_endpoint_if_unchanged(path: &Path, expected: EndpointIdentity) -> Result<bool> {
    let Some(actual) = endpoint_identity(path)? else {
        return Ok(false);
    };
    if actual != expected {
        anyhow::bail!(
            "daemon IPC endpoint at {path:?} changed while it was being checked; refusing to remove it"
        );
    }
    std::fs::remove_file(path)
        .with_context(|| format!("daemon IPC failed to remove verified socket at {path:?}"))?;
    Ok(true)
}

#[cfg(unix)]
async fn bind_unix_listener(
    path: &Path,
    probe_timeout: Duration,
) -> Result<(UnixListener, EndpointIdentity)> {
    prepare_endpoint_directory(path)?;
    if let Some(stale_candidate) = endpoint_identity(path)? {
        match timeout(probe_timeout, UnixStream::connect(path)).await {
            Ok(Ok(_stream)) => {
                anyhow::bail!("daemon already running: live IPC endpoint is listening at {path:?}");
            }
            Ok(Err(error)) if error.kind() == std::io::ErrorKind::ConnectionRefused => {
                remove_endpoint_if_unchanged(path, stale_candidate).with_context(|| {
                    format!("daemon IPC could not remove verified stale endpoint at {path:?}")
                })?;
            }
            Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {}
            Ok(Err(error)) => {
                return Err(anyhow::anyhow!(error).context(format!(
                    "daemon IPC endpoint conflict at {path:?}; refusing to replace it"
                )));
            }
            Err(_) => {
                anyhow::bail!(
                    "daemon IPC endpoint probe timed out at {path:?}; another daemon may be running, so the endpoint was not replaced"
                );
            }
        }
    }
    let listener = UnixListener::bind(path).with_context(|| {
        format!("daemon::runtime::ipc spawn_server failed to bind IPC socket at {path:?}")
    })?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("daemon IPC failed to secure socket permissions at {path:?}"))?;
    let identity = endpoint_identity(path)?.with_context(|| {
        format!("daemon IPC endpoint disappeared immediately after bind at {path:?}")
    })?;
    Ok((listener, identity))
}

#[cfg(unix)]
/// Expose status over a local IPC channel.
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
    let (listener, endpoint_identity) = bind_unix_listener(&socket_path, ipc_timeout).await?;
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
        drop(listener);
        if let Err(error) = remove_endpoint_if_unchanged(&socket_path, endpoint_identity) {
            warn!(
                component = "daemon",
                subsystem = "ipc",
                action = "cleanup_socket_failed",
                socket_path = %backup_core::logging::redact_path(&socket_path),
                error = %backup_core::logging::redact_text(&format!("{error:#}")),
                "daemon IPC socket cleanup skipped because endpoint ownership could not be verified"
            );
        }
    }))
}

#[cfg(windows)]
/// Expose status over a local IPC channel on Windows.
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
                    let connect_result = tokio::select! {
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
                        connected = timeout(ipc_timeout, server.connect()) => connected,
                    };
                    match connect_result {
                        Ok(Ok(())) => {}
                        Ok(Err(error)) => {
                            error!(
                                component = "daemon",
                                subsystem = "ipc",
                                action = "pipe_connect_failed",
                                error = %backup_core::logging::redact_text(&format!("{error:#}")),
                                "daemon IPC named-pipe connection failed"
                            );
                            continue;
                        }
                        Err(_) => {
                            warn!(
                                component = "daemon",
                                subsystem = "ipc",
                                action = "pipe_connect_timeout",
                                timeout_ms = ipc_timeout.as_millis() as u64,
                                "daemon IPC named-pipe connection wait timed out"
                            );
                            continue;
                        }
                    }
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

/// Centralize the IPC socket location in one helper.
pub fn socket_path() -> Result<PathBuf> {
    Ok(backup_core::platform::paths::ipc_socket_path())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
    async fn read_request_rejects_valid_oversized_json_received_in_one_chunk() {
        let (mut client, mut server) = tokio::io::duplex(4096);
        let payload = format!(
            r#"{{"type":"StatusWithContext","payload":{{"request_id":"req-oversized","source":"{}"}}}}"#,
            "x".repeat(256)
        );
        let max_request_bytes = payload.len() - 1;
        client.write_all(payload.as_bytes()).await.unwrap();

        let result = read_request(
            &mut server,
            Duration::from_secs(1),
            max_request_bytes,
            payload.len() + 1,
        )
        .await;

        let error = result.err().expect("oversized request must be rejected");
        assert!(
            format!("{error:#}").contains("exceeded max request size"),
            "unexpected error: {error:#}"
        );
    }

    #[tokio::test]
    async fn read_request_rejects_valid_oversized_json_received_in_fragments() {
        let (mut client, mut server) = tokio::io::duplex(4096);
        let payload = format!(
            r#"{{"type":"HealthWithContext","payload":{{"request_id":"health-oversized","source":"{}"}}}}"#,
            "x".repeat(256)
        );
        let max_request_bytes = 96;
        client.write_all(payload.as_bytes()).await.unwrap();

        let result = read_request(&mut server, Duration::from_secs(1), max_request_bytes, 7).await;

        let error = result.err().expect("oversized request must be rejected");
        assert!(
            format!("{error:#}").contains("exceeded max request size"),
            "unexpected error: {error:#}"
        );
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

    #[tokio::test]
    async fn live_endpoint_is_not_replaced_by_second_listener() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("ipc.sock");
        let (listener, identity) = bind_unix_listener(&path, Duration::from_secs(1))
            .await
            .unwrap();
        let permissions = std::fs::symlink_metadata(&path)
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(permissions, 0o600);
        let directory_permissions = std::fs::symlink_metadata(temp.path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(directory_permissions, 0o700);

        let error = bind_unix_listener(&path, Duration::from_secs(1))
            .await
            .unwrap_err();
        assert!(
            format!("{error:#}").contains("already running"),
            "unexpected conflict error: {error:#}"
        );
        assert_eq!(endpoint_identity(&path).unwrap(), Some(identity));

        drop(listener);
        assert!(remove_endpoint_if_unchanged(&path, identity).unwrap());
    }

    #[tokio::test]
    async fn non_socket_endpoint_is_never_deleted() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("ipc.sock");
        std::fs::write(&path, b"foreign data").unwrap();

        let error = bind_unix_listener(&path, Duration::from_secs(1))
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains("not a Unix socket"));
        assert_eq!(std::fs::read(&path).unwrap(), b"foreign data");
    }

    #[tokio::test]
    async fn verified_stale_socket_is_replaced() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("ipc.sock");
        let stale_listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        drop(stale_listener);

        let (listener, active_identity) = bind_unix_listener(&path, Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(endpoint_identity(&path).unwrap(), Some(active_identity));

        drop(listener);
        assert!(remove_endpoint_if_unchanged(&path, active_identity).unwrap());
    }

    #[tokio::test]
    async fn cleanup_refuses_to_remove_replacement_socket() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("ipc.sock");
        let (listener, original_identity) = bind_unix_listener(&path, Duration::from_secs(1))
            .await
            .unwrap();
        std::fs::remove_file(&path).unwrap();
        let replacement = std::os::unix::net::UnixListener::bind(&path).unwrap();

        let error = remove_endpoint_if_unchanged(&path, original_identity).unwrap_err();
        assert!(format!("{error:#}").contains("changed while it was being checked"));
        assert!(path.exists());

        drop(listener);
        drop(replacement);
        std::fs::remove_file(path).unwrap();
    }
}
