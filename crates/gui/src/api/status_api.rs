use anyhow::{anyhow, Context, Result};
use backup_core::config::model::RuntimeTuning;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::timeout;
use tracing::{info, warn};

#[derive(Deserialize, Debug, Clone)]
/// Summary: Status payload returned by the daemon IPC server.
///
/// Inputs: deserialized from IPC responses.
///
/// Outputs: a status structure used by the GUI layer.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI status queries.
///
/// Why this exists: mirror daemon status fields for the UI.
pub struct Status {
    pub last_run_ts: Option<i64>,
    pub last_files_backed_up: usize,
    pub last_error: Option<String>,
    pub last_dirty_count: usize,
    #[serde(default)]
    pub last_safety_warning: Option<backup_core::SafetyWarning>,
    pub uptime_secs: Option<i64>,
    pub version: Option<String>,
    pub free_bytes: Option<u64>,
    pub last_verify_ts: Option<i64>,
    pub last_verify_status: Option<String>,
    pub last_verify_issues: Option<usize>,
    pub recent_activity: Vec<backup_core::ActivityItem>,
    pub safe_mode: bool,
    #[serde(default)]
    pub destination_paused: bool,
    #[serde(default)]
    pub destination_pause_reason: Option<String>,
    #[serde(default)]
    pub destination_unavailable_ids: Vec<String>,
    #[serde(default)]
    pub destination_last_unavailable_ts: Option<i64>,
    #[serde(default)]
    pub destination_last_recovered_ts: Option<i64>,
    #[serde(default)]
    pub replication_last_run_ts: Option<i64>,
    #[serde(default)]
    pub replication_last_status: Option<String>,
    #[serde(default)]
    pub replication_last_error: Option<String>,
    #[serde(default)]
    pub replication_last_bytes_copied: u64,
    #[serde(default)]
    pub replication_last_blobs_copied: usize,
    #[serde(default)]
    pub replication_last_manifests_copied: usize,
    #[serde(default)]
    pub replication_last_manifests_deleted: usize,
    #[serde(default)]
    pub replication_last_pairs_ok: usize,
    #[serde(default)]
    pub replication_last_pairs_failed: usize,
    #[serde(default)]
    pub replication_last_targets_failed: Vec<String>,
    pub destinations: Vec<DestinationStatus>,
}

#[derive(Deserialize, Debug, Clone)]
/// Summary: Destination status payload returned by the daemon IPC server.
///
/// Inputs: deserialized from IPC responses.
///
/// Outputs: a destination status structure.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI status queries.
///
/// Why this exists: expose per destination free space and labels.
pub struct DestinationStatus {
    pub id: String,
    pub label: Option<String>,
    pub path: std::path::PathBuf,
    #[serde(default)]
    pub reachable: bool,
    #[serde(default)]
    pub writable: bool,
    pub free_bytes: Option<u64>,
    #[serde(default)]
    pub message: String,
}

const IPC_SOURCE_GUI: &str = "gui";
const METRIC_GUI_IPC_REQUEST_TOTAL: &str = "gui_ipc_request_total";
const METRIC_GUI_IPC_REQUEST_FAILURE_TOTAL: &str = "gui_ipc_request_failure_total";
const METRIC_GUI_IPC_REQUEST_LATENCY: &str = "gui_ipc_request_latency_ms";

#[derive(Serialize)]
#[serde(tag = "type", content = "payload")]
enum Request {
    StatusWithContext {
        request_id: String,
        source: String,
    },
    SetSafeMode {
        enabled: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<String>,
    },
    ClearSafetyWarningWithContext {
        request_id: String,
        source: String,
    },
}

#[derive(Deserialize)]
struct AckReply {
    ok: bool,
    #[allow(dead_code)]
    request_id: Option<String>,
}

/// Summary: Resolves an IPC request id from optional incoming correlation context.
///
/// Inputs: optional correlation id from GUI command boundaries.
///
/// Outputs: sanitized request id string.
///
/// Side effects: Reads time when generating fallback ids.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: IPC request serialization and structured logging.
///
/// Why this exists: keep GUI-to-daemon IPC correlation ids consistent and index-safe.
fn request_id(correlation_id: Option<&str>) -> String {
    backup_core::logging::correlation_id("gui-ipc", correlation_id)
}

/// Summary: Resolve the IPC timeout used for status calls.
///
/// Inputs: Reads config if available.
///
/// Outputs: A timeout duration for IPC operations.
///
/// Side effects: May read configuration from disk.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `fetch_status` connection and read/write time bounds.
///
/// Why this exists: Keep IPC calls bounded even when the daemon or filesystem misbehaves.
fn resolve_runtime_tuning() -> RuntimeTuning {
    match backup_core::load_validated_config() {
        Ok(cfg) => cfg.runtime,
        Err(error) => {
            warn!(
                error = %error,
                "gui::api::status_api::resolve_runtime_tuning failed loading config; using runtime defaults"
            );
            RuntimeTuning::default()
        }
    }
}

/// Summary: Resolves the status IPC timeout from runtime tuning.
///
/// Inputs: runtime tuning.
///
/// Outputs: timeout duration for IPC operations.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: status request/response connection and stream time bounds.
///
/// Why this exists: keep status IPC timeout logic centralized for all status endpoints.
fn resolve_ipc_timeout(runtime: &RuntimeTuning) -> Duration {
    Duration::from_secs(runtime.ipc_timeout_seconds.max(1))
}

/// Summary: Read an IPC response to EOF with a hard size cap.
///
/// Inputs: A readable stream and maximum byte limit.
///
/// Outputs: The collected bytes.
///
/// Side effects: Reads from the IPC stream until EOF or error.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `fetch_status_over_stream` response parsing.
///
/// Why this exists: Avoid unbounded memory growth on malformed or hostile IPC peers.
async fn read_bounded_to_end<R>(
    reader: &mut R,
    max_bytes: usize,
    read_chunk_bytes: usize,
    op_timeout: Duration,
) -> Result<Vec<u8>>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut buf = Vec::new();
    let mut chunk = vec![0u8; read_chunk_bytes.max(1)];
    loop {
        let n = timeout(op_timeout, reader.read(&mut chunk))
            .await
            .context("status_api::read_bounded_to_end timed out while reading")?
            .context("status_api::read_bounded_to_end failed while reading")?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.len() > max_bytes {
            return Err(anyhow!(
                "status_api::read_bounded_to_end response exceeded {} bytes",
                max_bytes
            ));
        }
    }
    Ok(buf)
}

/// Summary: Perform the status request/response exchange on an established IPC stream.
///
/// Inputs: A connected stream and an operation timeout.
///
/// Outputs: A deserialized `Status` value.
///
/// Side effects: Writes a JSON request, half-closes the write side, then reads a JSON reply.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `fetch_status` and the daemon's IPC server implementation.
///
/// Why this exists: Keep the on-the-wire protocol consistent and testable, and prevent request deadlocks.
async fn request_over_stream<S, T>(
    mut stream: S,
    request: &[u8],
    op_timeout: Duration,
    max_response_bytes: usize,
    read_chunk_bytes: usize,
) -> Result<T>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    T: DeserializeOwned,
{
    timeout(op_timeout, stream.write_all(request))
        .await
        .context("status_api::fetch_status_over_stream timed out writing status request")?
        .context("status_api::fetch_status_over_stream failed to write status request")?;

    // Signal EOF on the write half so the server can close the request stream promptly on all
    // platforms and the client can move to response reads without lingering writes.
    timeout(op_timeout, stream.shutdown())
        .await
        .context("status_api::fetch_status_over_stream timed out shutting down write half")?
        .context("status_api::fetch_status_over_stream failed shutting down write half")?;

    let buf = read_bounded_to_end(
        &mut stream,
        max_response_bytes,
        read_chunk_bytes,
        op_timeout,
    )
    .await?;
    if buf.is_empty() {
        return Err(anyhow!(
            "status_api::fetch_status_over_stream empty status response from daemon"
        ));
    }
    let raw = String::from_utf8_lossy(&buf);
    serde_json::from_slice(&buf).with_context(|| {
        format!(
            "status_api::fetch_status_over_stream failed to parse response: {}",
            raw
        )
    })
}

async fn fetch_status_over_stream<S>(
    stream: S,
    request: &[u8],
    op_timeout: Duration,
    max_response_bytes: usize,
    read_chunk_bytes: usize,
) -> Result<Status>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    request_over_stream(
        stream,
        request,
        op_timeout,
        max_response_bytes,
        read_chunk_bytes,
    )
    .await
}

#[cfg(windows)]
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::windows::named_pipe::ClientOptions,
};
#[cfg(unix)]
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
};

#[cfg(unix)]
/// Summary: Fetches status from the daemon over Unix IPC with correlation context.
///
/// Inputs: optional correlation id propagated from command boundaries.
///
/// Outputs: a deserialized `Status` value.
///
/// Side effects: Performs IPC over a Unix domain socket and emits structured telemetry.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI status refresh and daemon IPC request tracing.
///
/// Why this exists: preserve end-to-end request correlation from GUI to daemon logs.
pub async fn fetch_status_with_correlation(correlation_id: Option<&str>) -> Result<Status> {
    let runtime = resolve_runtime_tuning();
    let request_id = request_id(correlation_id);
    let request = serde_json::to_vec(&Request::StatusWithContext {
        request_id: request_id.clone(),
        source: IPC_SOURCE_GUI.to_string(),
    })
    .context("status_api::fetch_status_with_correlation failed to serialize status request")?;
    let socket = super::socket_path()?;
    if !socket.exists() {
        backup_core::metrics::counter_inc(METRIC_GUI_IPC_REQUEST_FAILURE_TOTAL, 1);
        return Err(anyhow!(
            "status_api::fetch_status daemon IPC socket not found at {:?}",
            socket
        ));
    }
    backup_core::metrics::counter_inc(METRIC_GUI_IPC_REQUEST_TOTAL, 1);
    let started = std::time::Instant::now();
    let op_timeout = resolve_ipc_timeout(&runtime);
    let stream = timeout(op_timeout, UnixStream::connect(&socket))
        .await
        .with_context(|| {
            format!(
                "status_api::fetch_status timed out connecting to daemon socket {:?}",
                socket
            )
        })?
        .with_context(|| format!("status_api::fetch_status failed to connect to {:?}", socket))?;
    let result = fetch_status_over_stream(
        stream,
        &request,
        op_timeout,
        runtime.ipc_response_max_bytes,
        runtime.ipc_read_chunk_bytes,
    )
    .await;
    backup_core::metrics::observe_duration(METRIC_GUI_IPC_REQUEST_LATENCY, started.elapsed());
    match result {
        Ok(status) => {
            info!(
                component = "gui",
                subsystem = "ipc_client",
                action = "status_fetch_succeeded",
                request_id = %request_id,
                source = IPC_SOURCE_GUI,
                "GUI IPC status fetch succeeded"
            );
            Ok(status)
        }
        Err(error) => {
            backup_core::metrics::counter_inc(METRIC_GUI_IPC_REQUEST_FAILURE_TOTAL, 1);
            warn!(
                component = "gui",
                subsystem = "ipc_client",
                action = "status_fetch_failed",
                request_id = %request_id,
                source = IPC_SOURCE_GUI,
                error = %error,
                "GUI IPC status fetch failed"
            );
            Err(error)
        }
    }
}

#[cfg(unix)]
pub async fn set_safe_mode_with_correlation(
    enabled: bool,
    correlation_id: Option<&str>,
) -> Result<()> {
    let runtime = resolve_runtime_tuning();
    let request_id = request_id(correlation_id);
    let socket = super::socket_path()?;
    if !socket.exists() {
        backup_core::metrics::counter_inc(METRIC_GUI_IPC_REQUEST_FAILURE_TOTAL, 1);
        return Err(anyhow!(
            "status_api::set_safe_mode daemon IPC socket not found at {:?}",
            socket
        ));
    }
    backup_core::metrics::counter_inc(METRIC_GUI_IPC_REQUEST_TOTAL, 1);
    let started = std::time::Instant::now();
    let op_timeout = resolve_ipc_timeout(&runtime);
    let stream = timeout(op_timeout, UnixStream::connect(&socket))
        .await
        .with_context(|| {
            format!(
                "status_api::set_safe_mode timed out connecting to daemon socket {:?}",
                socket
            )
        })?
        .with_context(|| {
            format!(
                "status_api::set_safe_mode failed to connect to {:?}",
                socket
            )
        })?;

    let request = serde_json::to_vec(&Request::SetSafeMode {
        enabled,
        request_id: Some(request_id.clone()),
        source: Some(IPC_SOURCE_GUI.to_string()),
    })
    .context("status_api::set_safe_mode failed to serialize request")?;
    let result: Result<AckReply> = request_over_stream(
        stream,
        &request,
        op_timeout,
        runtime.ipc_response_max_bytes,
        runtime.ipc_read_chunk_bytes,
    )
    .await;
    backup_core::metrics::observe_duration(METRIC_GUI_IPC_REQUEST_LATENCY, started.elapsed());
    match result {
        Ok(ack) => {
            if !ack.ok {
                backup_core::metrics::counter_inc(METRIC_GUI_IPC_REQUEST_FAILURE_TOTAL, 1);
                return Err(anyhow!(
                    "status_api::set_safe_mode daemon returned ok=false"
                ));
            }
            info!(
                component = "gui",
                subsystem = "ipc_client",
                action = "set_safe_mode_succeeded",
                request_id = %request_id,
                source = IPC_SOURCE_GUI,
                enabled = enabled,
                "GUI IPC set-safe-mode succeeded"
            );
            Ok(())
        }
        Err(error) => {
            backup_core::metrics::counter_inc(METRIC_GUI_IPC_REQUEST_FAILURE_TOTAL, 1);
            warn!(
                component = "gui",
                subsystem = "ipc_client",
                action = "set_safe_mode_failed",
                request_id = %request_id,
                source = IPC_SOURCE_GUI,
                enabled = enabled,
                error = %error,
                "GUI IPC set-safe-mode failed"
            );
            Err(error)
        }
    }
}

#[cfg(windows)]
pub async fn set_safe_mode_with_correlation(
    enabled: bool,
    correlation_id: Option<&str>,
) -> Result<()> {
    let runtime = resolve_runtime_tuning();
    let request_id = request_id(correlation_id);
    backup_core::metrics::counter_inc(METRIC_GUI_IPC_REQUEST_TOTAL, 1);
    let started = std::time::Instant::now();
    let op_timeout = resolve_ipc_timeout(&runtime);
    let mut stream = ClientOptions::new()
        .open(r"\\.\pipe\backup_sync_ipc")
        .with_context(|| {
            "status_api::set_safe_mode failed to connect to named pipe \\\\.\\pipe\\backup_sync_ipc"
        })?;
    let request = serde_json::to_vec(&Request::SetSafeMode {
        enabled,
        request_id: Some(request_id.clone()),
        source: Some(IPC_SOURCE_GUI.to_string()),
    })
    .context("status_api::set_safe_mode failed to serialize request")?;
    let result: Result<AckReply> = request_over_stream(
        &mut stream,
        &request,
        op_timeout,
        runtime.ipc_response_max_bytes,
        runtime.ipc_read_chunk_bytes,
    )
    .await;
    backup_core::metrics::observe_duration(METRIC_GUI_IPC_REQUEST_LATENCY, started.elapsed());
    match result {
        Ok(ack) => {
            if !ack.ok {
                backup_core::metrics::counter_inc(METRIC_GUI_IPC_REQUEST_FAILURE_TOTAL, 1);
                return Err(anyhow!(
                    "status_api::set_safe_mode daemon returned ok=false"
                ));
            }
            info!(
                component = "gui",
                subsystem = "ipc_client",
                action = "set_safe_mode_succeeded",
                request_id = %request_id,
                source = IPC_SOURCE_GUI,
                enabled = enabled,
                "GUI IPC set-safe-mode succeeded"
            );
            Ok(())
        }
        Err(error) => {
            backup_core::metrics::counter_inc(METRIC_GUI_IPC_REQUEST_FAILURE_TOTAL, 1);
            warn!(
                component = "gui",
                subsystem = "ipc_client",
                action = "set_safe_mode_failed",
                request_id = %request_id,
                source = IPC_SOURCE_GUI,
                enabled = enabled,
                error = %error,
                "GUI IPC set-safe-mode failed"
            );
            Err(error)
        }
    }
}

#[cfg(unix)]
pub async fn clear_safety_warning_with_correlation(correlation_id: Option<&str>) -> Result<()> {
    let runtime = resolve_runtime_tuning();
    let request_id = request_id(correlation_id);
    let socket = super::socket_path()?;
    if !socket.exists() {
        backup_core::metrics::counter_inc(METRIC_GUI_IPC_REQUEST_FAILURE_TOTAL, 1);
        return Err(anyhow!(
            "status_api::clear_safety_warning daemon IPC socket not found at {:?}",
            socket
        ));
    }
    backup_core::metrics::counter_inc(METRIC_GUI_IPC_REQUEST_TOTAL, 1);
    let started = std::time::Instant::now();
    let op_timeout = resolve_ipc_timeout(&runtime);
    let stream = timeout(op_timeout, UnixStream::connect(&socket))
        .await
        .with_context(|| {
            format!(
                "status_api::clear_safety_warning timed out connecting to daemon socket {:?}",
                socket
            )
        })?
        .with_context(|| {
            format!(
                "status_api::clear_safety_warning failed to connect to {:?}",
                socket
            )
        })?;

    let request = serde_json::to_vec(&Request::ClearSafetyWarningWithContext {
        request_id: request_id.clone(),
        source: IPC_SOURCE_GUI.to_string(),
    })
    .context("status_api::clear_safety_warning failed to serialize request")?;
    let result: Result<AckReply> = request_over_stream(
        stream,
        &request,
        op_timeout,
        runtime.ipc_response_max_bytes,
        runtime.ipc_read_chunk_bytes,
    )
    .await;
    backup_core::metrics::observe_duration(METRIC_GUI_IPC_REQUEST_LATENCY, started.elapsed());
    match result {
        Ok(ack) => {
            if !ack.ok {
                backup_core::metrics::counter_inc(METRIC_GUI_IPC_REQUEST_FAILURE_TOTAL, 1);
                return Err(anyhow!(
                    "status_api::clear_safety_warning daemon returned ok=false"
                ));
            }
            info!(
                component = "gui",
                subsystem = "ipc_client",
                action = "clear_safety_warning_succeeded",
                request_id = %request_id,
                source = IPC_SOURCE_GUI,
                "GUI IPC clear-safety-warning succeeded"
            );
            Ok(())
        }
        Err(error) => {
            backup_core::metrics::counter_inc(METRIC_GUI_IPC_REQUEST_FAILURE_TOTAL, 1);
            warn!(
                component = "gui",
                subsystem = "ipc_client",
                action = "clear_safety_warning_failed",
                request_id = %request_id,
                source = IPC_SOURCE_GUI,
                error = %error,
                "GUI IPC clear-safety-warning failed"
            );
            Err(error)
        }
    }
}

#[cfg(windows)]
pub async fn clear_safety_warning_with_correlation(correlation_id: Option<&str>) -> Result<()> {
    let runtime = resolve_runtime_tuning();
    let request_id = request_id(correlation_id);
    backup_core::metrics::counter_inc(METRIC_GUI_IPC_REQUEST_TOTAL, 1);
    let started = std::time::Instant::now();
    let op_timeout = resolve_ipc_timeout(&runtime);
    let mut stream = ClientOptions::new()
        .open(r"\\.\pipe\backup_sync_ipc")
        .with_context(|| {
            "status_api::clear_safety_warning failed to connect to named pipe \\\\.\\pipe\\backup_sync_ipc"
        })?;
    let request = serde_json::to_vec(&Request::ClearSafetyWarningWithContext {
        request_id: request_id.clone(),
        source: IPC_SOURCE_GUI.to_string(),
    })
    .context("status_api::clear_safety_warning failed to serialize request")?;
    let result: Result<AckReply> = request_over_stream(
        &mut stream,
        &request,
        op_timeout,
        runtime.ipc_response_max_bytes,
        runtime.ipc_read_chunk_bytes,
    )
    .await;
    backup_core::metrics::observe_duration(METRIC_GUI_IPC_REQUEST_LATENCY, started.elapsed());
    match result {
        Ok(ack) => {
            if !ack.ok {
                backup_core::metrics::counter_inc(METRIC_GUI_IPC_REQUEST_FAILURE_TOTAL, 1);
                return Err(anyhow!(
                    "status_api::clear_safety_warning daemon returned ok=false"
                ));
            }
            info!(
                component = "gui",
                subsystem = "ipc_client",
                action = "clear_safety_warning_succeeded",
                request_id = %request_id,
                source = IPC_SOURCE_GUI,
                "GUI IPC clear-safety-warning succeeded"
            );
            Ok(())
        }
        Err(error) => {
            backup_core::metrics::counter_inc(METRIC_GUI_IPC_REQUEST_FAILURE_TOTAL, 1);
            warn!(
                component = "gui",
                subsystem = "ipc_client",
                action = "clear_safety_warning_failed",
                request_id = %request_id,
                source = IPC_SOURCE_GUI,
                error = %error,
                "GUI IPC clear-safety-warning failed"
            );
            Err(error)
        }
    }
}

#[cfg(windows)]
pub async fn fetch_status_with_correlation(correlation_id: Option<&str>) -> Result<Status> {
    let runtime = resolve_runtime_tuning();
    let request_id = request_id(correlation_id);
    let request = serde_json::to_vec(&Request::StatusWithContext {
        request_id: request_id.clone(),
        source: IPC_SOURCE_GUI.to_string(),
    })
    .context("status_api::fetch_status_with_correlation failed to serialize status request")?;
    backup_core::metrics::counter_inc(METRIC_GUI_IPC_REQUEST_TOTAL, 1);
    let started = std::time::Instant::now();
    let op_timeout = resolve_ipc_timeout(&runtime);
    let mut stream = ClientOptions::new()
        .open(r"\\.\pipe\backup_sync_ipc")
        .with_context(|| {
            "status_api::fetch_status failed to connect to named pipe \\\\.\\pipe\\backup_sync_ipc"
        })?;
    let result = fetch_status_over_stream(
        &mut stream,
        &request,
        op_timeout,
        runtime.ipc_response_max_bytes,
        runtime.ipc_read_chunk_bytes,
    )
    .await;
    backup_core::metrics::observe_duration(METRIC_GUI_IPC_REQUEST_LATENCY, started.elapsed());
    match result {
        Ok(status) => {
            info!(
                component = "gui",
                subsystem = "ipc_client",
                action = "status_fetch_succeeded",
                request_id = %request_id,
                source = IPC_SOURCE_GUI,
                "GUI IPC status fetch succeeded"
            );
            Ok(status)
        }
        Err(error) => {
            backup_core::metrics::counter_inc(METRIC_GUI_IPC_REQUEST_FAILURE_TOTAL, 1);
            warn!(
                component = "gui",
                subsystem = "ipc_client",
                action = "status_fetch_failed",
                request_id = %request_id,
                source = IPC_SOURCE_GUI,
                error = %error,
                "GUI IPC status fetch failed"
            );
            Err(error)
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn request_id_sanitizes_untrusted_input() {
        let cid = request_id(Some(" gui\tcid\n "));
        assert_eq!(cid, "gui_cid");
    }

    /// Summary: fetch_status_over_stream_signals_eof_to_avoid_deadlock orchestrates this method's core behavior.
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
    async fn fetch_status_over_stream_signals_eof_to_avoid_deadlock() {
        let (mut client, mut server) = tokio::net::UnixStream::pair().unwrap();
        let runtime = RuntimeTuning::default();
        let request = serde_json::to_vec(&Request::StatusWithContext {
            request_id: "test-cid".to_string(),
            source: IPC_SOURCE_GUI.to_string(),
        })
        .unwrap();
        let server_task = tokio::spawn(async move {
            // Simulate the daemon behavior: read to EOF before responding.
            let mut req = Vec::new();
            server.read_to_end(&mut req).await.unwrap();
            assert_eq!(req, request);
            let reply = br#"{"last_run_ts":null,"last_files_backed_up":0,"last_error":null,"last_dirty_count":0,"uptime_secs":null,"version":null,"free_bytes":null,"last_verify_ts":null,"last_verify_status":null,"last_verify_issues":null,"recent_activity":[],"safe_mode":false,"destinations":[]}"#;
            server.write_all(reply).await.unwrap();
            server
                .shutdown()
                .await
                .expect("status_api::tests::fetch_status_over_stream_signals_eof_to_avoid_deadlock failed shutting down server stream");
        });

        let st = fetch_status_over_stream(
            &mut client,
            &serde_json::to_vec(&Request::StatusWithContext {
                request_id: "test-cid".to_string(),
                source: IPC_SOURCE_GUI.to_string(),
            })
            .unwrap(),
            Duration::from_secs(2),
            runtime.ipc_response_max_bytes,
            runtime.ipc_read_chunk_bytes,
        )
        .await
        .unwrap();
        assert_eq!(st.last_files_backed_up, 0);
        server_task.await.unwrap();
    }
}
