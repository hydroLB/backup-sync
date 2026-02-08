use anyhow::{anyhow, Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::timeout;

#[derive(Deserialize, Debug, Clone)]
/// Purpose: Status payload returned by the daemon IPC server.
///
/// Inputs: deserialized from IPC responses.
/// Outputs: a status structure used by the GUI layer.
/// Ties to: GUI status queries.
/// Side effects: None.
/// Why: mirror daemon status fields for the UI.
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
/// Purpose: Destination status payload returned by the daemon IPC server.
///
/// Inputs: deserialized from IPC responses.
/// Outputs: a destination status structure.
/// Ties to: GUI status queries.
/// Side effects: None.
/// Why: expose per destination free space and labels.
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

const DEFAULT_IPC_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_IPC_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const STATUS_REQUEST_BYTES: &[u8] = br#"{"type":"Status"}"#;

#[derive(Serialize)]
#[serde(tag = "type", content = "payload")]
enum Request {
    SetSafeMode { enabled: bool },
    ClearSafetyWarning,
}

#[derive(Deserialize)]
struct AckReply {
    ok: bool,
}

/// Purpose: Resolve the IPC timeout used for status calls.
///
/// Inputs: Reads config if available.
/// Outputs: A timeout duration for IPC operations.
/// Ties to: `fetch_status` connection and read/write time bounds.
/// Side effects: May read configuration from disk.
/// Why: Keep IPC calls bounded even when the daemon or filesystem misbehaves.
fn resolve_ipc_timeout() -> Duration {
    match backup_core::load_config() {
        Ok(cfg) => Duration::from_secs(cfg.runtime.ipc_timeout_seconds.max(1)),
        Err(_) => DEFAULT_IPC_TIMEOUT,
    }
}

/// Purpose: Read an IPC response to EOF with a hard size cap.
///
/// Inputs: A readable stream and maximum byte limit.
/// Outputs: The collected bytes.
/// Ties to: `fetch_status_over_stream` response parsing.
/// Side effects: Reads from the IPC stream until EOF or error.
/// Why: Avoid unbounded memory growth on malformed or hostile IPC peers.
async fn read_bounded_to_end<R>(
    reader: &mut R,
    max_bytes: usize,
    op_timeout: Duration,
) -> Result<Vec<u8>>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
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

/// Purpose: Perform the status request/response exchange on an established IPC stream.
///
/// Inputs: A connected stream and an operation timeout.
/// Outputs: A deserialized `Status` value.
/// Ties to: `fetch_status` and the daemon's IPC server implementation.
/// Side effects: Writes a JSON request, half-closes the write side, then reads a JSON reply.
/// Why: Keep the on-the-wire protocol consistent and testable, and prevent request deadlocks.
async fn request_over_stream<S, T>(mut stream: S, request: &[u8], op_timeout: Duration) -> Result<T>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    T: DeserializeOwned,
{
    timeout(op_timeout, stream.write_all(request))
        .await
        .context("status_api::fetch_status_over_stream timed out writing status request")?
        .context("status_api::fetch_status_over_stream failed to write status request")?;

    // The daemon reads the request using `read_to_end`, so the client must signal EOF on the write
    // half to avoid both sides waiting indefinitely.
    timeout(op_timeout, stream.shutdown())
        .await
        .context("status_api::fetch_status_over_stream timed out shutting down write half")?
        .ok();

    let buf = read_bounded_to_end(&mut stream, MAX_IPC_RESPONSE_BYTES, op_timeout).await?;
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

async fn fetch_status_over_stream<S>(stream: S, op_timeout: Duration) -> Result<Status>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    request_over_stream(stream, STATUS_REQUEST_BYTES, op_timeout).await
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
/// Purpose: Fetches status from the daemon over a Unix domain socket.
///
/// Inputs: none.
/// Outputs: a deserialized `Status` value.
/// Ties to: GUI status refresh flows on Unix.
/// Side effects: Performs IPC over a Unix domain socket.
/// Why: provide up to date daemon status to the UI.
pub async fn fetch_status() -> Result<Status> {
    let socket = super::socket_path()?;
    if !socket.exists() {
        return Err(anyhow!(
            "status_api::fetch_status daemon IPC socket not found at {:?}",
            socket
        ));
    }
    let op_timeout = resolve_ipc_timeout();
    let stream = timeout(op_timeout, UnixStream::connect(&socket))
        .await
        .with_context(|| {
            format!(
                "status_api::fetch_status timed out connecting to daemon socket {:?}",
                socket
            )
        })?
        .with_context(|| format!("status_api::fetch_status failed to connect to {:?}", socket))?;
    fetch_status_over_stream(stream, op_timeout).await
}

#[cfg(unix)]
pub async fn set_safe_mode(enabled: bool) -> Result<()> {
    let socket = super::socket_path()?;
    if !socket.exists() {
        return Err(anyhow!(
            "status_api::set_safe_mode daemon IPC socket not found at {:?}",
            socket
        ));
    }
    let op_timeout = resolve_ipc_timeout();
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

    let request = serde_json::to_vec(&Request::SetSafeMode { enabled })
        .context("status_api::set_safe_mode failed to serialize request")?;
    let ack: AckReply = request_over_stream(stream, &request, op_timeout).await?;
    if !ack.ok {
        return Err(anyhow!(
            "status_api::set_safe_mode daemon returned ok=false"
        ));
    }
    Ok(())
}

#[cfg(windows)]
pub async fn set_safe_mode(enabled: bool) -> Result<()> {
    let op_timeout = resolve_ipc_timeout();
    let mut stream = ClientOptions::new()
        .open(r"\\.\pipe\backup_sync_ipc")
        .with_context(|| {
            "status_api::set_safe_mode failed to connect to named pipe \\\\.\\pipe\\backup_sync_ipc"
        })?;
    let request = serde_json::to_vec(&Request::SetSafeMode { enabled })
        .context("status_api::set_safe_mode failed to serialize request")?;
    let ack: AckReply = request_over_stream(&mut stream, &request, op_timeout).await?;
    if !ack.ok {
        return Err(anyhow!(
            "status_api::set_safe_mode daemon returned ok=false"
        ));
    }
    Ok(())
}

#[cfg(unix)]
pub async fn clear_safety_warning() -> Result<()> {
    let socket = super::socket_path()?;
    if !socket.exists() {
        return Err(anyhow!(
            "status_api::clear_safety_warning daemon IPC socket not found at {:?}",
            socket
        ));
    }
    let op_timeout = resolve_ipc_timeout();
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

    let request = serde_json::to_vec(&Request::ClearSafetyWarning)
        .context("status_api::clear_safety_warning failed to serialize request")?;
    let ack: AckReply = request_over_stream(stream, &request, op_timeout).await?;
    if !ack.ok {
        return Err(anyhow!(
            "status_api::clear_safety_warning daemon returned ok=false"
        ));
    }
    Ok(())
}

#[cfg(windows)]
pub async fn clear_safety_warning() -> Result<()> {
    let op_timeout = resolve_ipc_timeout();
    let mut stream = ClientOptions::new()
        .open(r"\\.\pipe\backup_sync_ipc")
        .with_context(|| {
            "status_api::clear_safety_warning failed to connect to named pipe \\\\.\\pipe\\backup_sync_ipc"
        })?;
    let request = serde_json::to_vec(&Request::ClearSafetyWarning)
        .context("status_api::clear_safety_warning failed to serialize request")?;
    let ack: AckReply = request_over_stream(&mut stream, &request, op_timeout).await?;
    if !ack.ok {
        return Err(anyhow!(
            "status_api::clear_safety_warning daemon returned ok=false"
        ));
    }
    Ok(())
}

#[cfg(windows)]
/// Purpose: Fetches status from the daemon over a Windows named pipe.
///
/// Inputs: none.
/// Outputs: a deserialized `Status` value.
/// Ties to: GUI status refresh flows on Windows.
/// Side effects: Performs IPC over a Windows named pipe.
/// Why: provide up to date daemon status to the UI.
pub async fn fetch_status() -> Result<Status> {
    let op_timeout = resolve_ipc_timeout();
    let mut stream = ClientOptions::new()
        .open(r"\\.\pipe\backup_sync_ipc")
        .with_context(|| {
            "status_api::fetch_status failed to connect to named pipe \\\\.\\pipe\\backup_sync_ipc"
        })?;
    // Windows named pipes implement AsyncRead/AsyncWrite; reuse the shared protocol handler.
    fetch_status_over_stream(&mut stream, op_timeout).await
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn fetch_status_over_stream_signals_eof_to_avoid_deadlock() {
        let (mut client, mut server) = tokio::net::UnixStream::pair().unwrap();
        let server_task = tokio::spawn(async move {
            // Simulate the daemon behavior: read to EOF before responding.
            let mut req = Vec::new();
            server.read_to_end(&mut req).await.unwrap();
            assert_eq!(req, STATUS_REQUEST_BYTES);
            let reply = br#"{"last_run_ts":null,"last_files_backed_up":0,"last_error":null,"last_dirty_count":0,"uptime_secs":null,"version":null,"free_bytes":null,"last_verify_ts":null,"last_verify_status":null,"last_verify_issues":null,"recent_activity":[],"safe_mode":false,"destinations":[]}"#;
            server.write_all(reply).await.unwrap();
            server.shutdown().await.ok();
        });

        let st = fetch_status_over_stream(&mut client, Duration::from_secs(2))
            .await
            .unwrap();
        assert_eq!(st.last_files_backed_up, 0);
        server_task.await.unwrap();
    }
}
