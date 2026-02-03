use anyhow::{Context, Result};
use backup_core::StoredState;
use chrono::Utc;
use fs2::free_space;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::Mutex;
use tokio::time::timeout;
use tracing::{info, warn};

use std::time::Duration;
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
    SetSafeMode { enabled: bool },
}

const MAX_REQUEST_BYTES: usize = 64 * 1024;

#[derive(Serialize, Deserialize, Clone)]
struct DestinationStatus {
    id: String,
    label: Option<String>,
    path: PathBuf,
    free_bytes: Option<u64>,
}

#[derive(Serialize, Deserialize)]
struct StatusReply {
    last_run_ts: Option<i64>,
    last_files_backed_up: usize,
    last_error: Option<String>,
    last_dirty_count: usize,
    uptime_secs: Option<i64>,
    version: Option<String>,
    free_bytes: Option<u64>,
    last_verify_ts: Option<i64>,
    last_verify_status: Option<String>,
    last_verify_issues: Option<usize>,
    recent_activity: Vec<backup_core::ActivityItem>,
    safe_mode: bool,
    destinations: Vec<DestinationStatus>,
}

#[derive(Serialize)]
struct AckReply {
    ok: bool,
}

/// Purpose: Reads free space for a path, logging warnings when it fails.
///
/// Inputs: the path to check.
/// Outputs: an optional free space value.
/// Ties to: IPC status payload construction.
/// Side effects: Reads filesystem free space and emits warnings on failure.
/// Why: report free space when available without failing the IPC response.
fn free_space_or_warn(path: &Path) -> Option<u64> {
    match free_space(path) {
        Ok(bytes) => Some(bytes),
        Err(e) => {
            warn!(
                "daemon::runtime::ipc free_space_or_warn failed for {:?}: {}",
                path, e
            );
            None
        }
    }
}

/// Purpose: Builds an IPC status reply from stored state and destination metadata.
///
/// Inputs: the stored state, destination list, and activity limit.
/// Outputs: a `StatusReply` ready for serialization.
/// Ties to: IPC request handling for status calls.
/// Side effects: Reads filesystem free space metadata for destinations.
/// Why: centralize status payload construction for the UI and CLI.
fn build_status_reply(
    state: &StoredState,
    destinations: &[backup_core::config::model::Destination],
    recent_activity_limit: usize,
) -> StatusReply {
    let uptime_secs = state.start_ts.map(|ts| Utc::now().timestamp() - ts);
    let dest_status: Vec<_> = destinations
        .iter()
        .map(|d| DestinationStatus {
            id: d.id.clone(),
            label: d.label.clone(),
            path: d.path.clone(),
            free_bytes: free_space_or_warn(&d.path),
        })
        .collect();
    let free_bytes = dest_status.first().and_then(|d| d.free_bytes);
    StatusReply {
        last_run_ts: state.last_run_ts,
        last_files_backed_up: state.last_files_backed_up,
        last_error: state.last_error.clone(),
        last_dirty_count: state.last_dirty_count,
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
        destinations: dest_status,
    }
}

/// Purpose: Writes a serialized status reply to the IPC stream and closes it.
///
/// Inputs: a writable stream and the status reply.
/// Outputs: `Ok(())` when the reply is written and the stream is closed.
/// Ties to: IPC status request handling.
/// Side effects: Writes to the IPC stream and shuts it down.
/// Why: ensure IPC responses complete cleanly for clients.
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
    writer.shutdown().await.ok();
    Ok(())
}

async fn write_ack_reply<W>(writer: &mut W, ok: bool) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let payload = serde_json::to_string(&AckReply { ok })
        .context("daemon::runtime::ipc write_ack_reply failed to serialize reply")?;
    writer
        .write_all(payload.as_bytes())
        .await
        .context("daemon::runtime::ipc write_ack_reply failed to write reply")?;
    writer.shutdown().await.ok();
    Ok(())
}

/// Purpose: Reads and parses a JSON IPC request without requiring the client to close the stream.
///
/// Inputs: a readable IPC stream, a timeout bound, and a max request size.
/// Outputs: a parsed `Request` value.
/// Ties to: both Unix socket and Windows named pipe IPC servers.
/// Side effects: Reads bytes from the IPC stream until a request is parsed or limits are exceeded.
/// Why: Avoid deadlocks where both sides wait for EOF; allow small request/response exchanges.
async fn read_request<R>(reader: &mut R, ipc_timeout: Duration) -> Result<Request>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 2048];
    loop {
        if buf.len() > MAX_REQUEST_BYTES {
            anyhow::bail!(
                "daemon::runtime::ipc read_request exceeded max request size {} bytes",
                MAX_REQUEST_BYTES
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
/// Purpose: Spawns a Unix socket based IPC server for status requests.
///
/// Inputs: shared state, destination list, IPC timeout, and activity limit.
/// Outputs: a join handle for the IPC task.
/// Ties to: daemon IPC handling for GUI and CLI clients.
/// Side effects: Binds a Unix socket and performs IPC reads and writes.
/// Why: expose status over a local IPC channel.
pub async fn spawn_server(
    state: Arc<Mutex<StoredState>>,
    destinations: Vec<backup_core::config::model::Destination>,
    ipc_timeout: Duration,
    recent_activity_limit: usize,
) -> Result<tokio::task::JoinHandle<()>> {
    let socket_path = socket_path()?;
    if socket_path.exists() {
        if let Err(e) = std::fs::remove_file(&socket_path) {
            warn!(
                "daemon::runtime::ipc spawn_server failed to remove existing socket {:?}: {}",
                socket_path, e
            );
        }
    }
    let listener = UnixListener::bind(&socket_path).with_context(|| {
        format!(
            "daemon::runtime::ipc spawn_server failed to bind IPC socket at {:?}",
            socket_path
        )
    })?;
    info!("IPC listening at {:?}", socket_path);
    Ok(tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((mut stream, _addr)) => {
                    let req = match read_request(&mut stream, ipc_timeout).await {
                        Ok(r) => r,
                        Err(e) => {
                            tracing::error!(
                                "daemon::runtime::ipc spawn_server request read error: {e:?}"
                            );
                            continue;
                        }
                    };
                    match req {
                        Request::Status => {
                            let st = state.lock().await.clone();
                            let reply =
                                build_status_reply(&st, &destinations, recent_activity_limit);
                            match timeout(ipc_timeout, write_status_reply(&mut stream, &reply))
                                .await
                            {
                                Ok(Ok(_)) => {}
                                Ok(Err(e)) => {
                                    tracing::error!(
                                        "daemon::runtime::ipc spawn_server write error: {e:?}"
                                    );
                                }
                                Err(_) => {
                                    tracing::error!(
                                        "daemon::runtime::ipc spawn_server write timeout after {:?}",
                                        ipc_timeout
                                    );
                                }
                            }
                        }
                        Request::SetSafeMode { enabled } => {
                            {
                                let mut st = state.lock().await;
                                st.safe_mode = enabled;
                            }
                            match timeout(ipc_timeout, write_ack_reply(&mut stream, true)).await {
                                Ok(Ok(_)) => {}
                                Ok(Err(e)) => {
                                    tracing::error!(
                                        "daemon::runtime::ipc spawn_server write error: {e:?}"
                                    );
                                }
                                Err(_) => {
                                    tracing::error!(
                                        "daemon::runtime::ipc spawn_server write timeout after {:?}",
                                        ipc_timeout
                                    );
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("daemon::runtime::ipc spawn_server accept error: {e:?}");
                    break;
                }
            }
        }
    }))
}

#[cfg(windows)]
/// Purpose: Spawns a Windows named pipe IPC server for status requests.
///
/// Inputs: shared state, destination list, IPC timeout, and activity limit.
/// Outputs: a join handle for the IPC task.
/// Ties to: daemon IPC handling for GUI and CLI clients.
/// Side effects: Creates a named pipe and performs IPC reads and writes.
/// Why: expose status over a local IPC channel on Windows.
pub async fn spawn_server(
    _state: Arc<Mutex<StoredState>>,
    destinations: Vec<backup_core::config::model::Destination>,
    ipc_timeout: Duration,
    recent_activity_limit: usize,
) -> Result<tokio::task::JoinHandle<()>> {
    let pipe_name = r"\\.\pipe\backup_sync_ipc";
    let state = _state;
    Ok(tokio::spawn(async move {
        loop {
            match ServerOptions::new()
                .first_pipe_instance(true)
                .create(pipe_name)
            {
                Ok(mut server) => {
                    let req = match read_request(&mut server, ipc_timeout).await {
                        Ok(r) => r,
                        Err(e) => {
                            tracing::error!(
                                "daemon::runtime::ipc spawn_server request read error: {e:?}"
                            );
                            continue;
                        }
                    };
                    match req {
                        Request::Status => {
                            let st = state.lock().await.clone();
                            let reply =
                                build_status_reply(&st, &destinations, recent_activity_limit);
                            match timeout(ipc_timeout, write_status_reply(&mut server, &reply))
                                .await
                            {
                                Ok(Ok(_)) => {}
                                Ok(Err(e)) => {
                                    tracing::error!(
                                        "daemon::runtime::ipc spawn_server write error: {e:?}"
                                    );
                                }
                                Err(_) => {
                                    tracing::error!(
                                        "daemon::runtime::ipc spawn_server write timeout after {:?}",
                                        ipc_timeout
                                    );
                                }
                            }
                        }
                        Request::SetSafeMode { enabled } => {
                            {
                                let mut st = state.lock().await;
                                st.safe_mode = enabled;
                            }
                            match timeout(ipc_timeout, write_ack_reply(&mut server, true)).await {
                                Ok(Ok(_)) => {}
                                Ok(Err(e)) => {
                                    tracing::error!(
                                        "daemon::runtime::ipc spawn_server write error: {e:?}"
                                    );
                                }
                                Err(_) => {
                                    tracing::error!(
                                        "daemon::runtime::ipc spawn_server write timeout after {:?}",
                                        ipc_timeout
                                    );
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "daemon::runtime::ipc spawn_server pipe accept error: {:?}",
                        e
                    );
                    break;
                }
            }
        }
    }))
}

/// Purpose: Builds the Unix socket path used for IPC.
///
/// Inputs: none.
/// Outputs: the filesystem path for the IPC socket.
/// Ties to: IPC server setup on Unix platforms.
/// Side effects: Reads the runtime directory or temp directory.
/// Why: centralize the IPC socket location in one helper.
pub fn socket_path() -> Result<PathBuf> {
    let mut p = dirs::runtime_dir().unwrap_or(std::env::temp_dir());
    p.push("backup_sync_ipc.sock");
    Ok(p)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn read_request_parses_without_client_shutdown() {
        let (mut client, mut server) = tokio::net::UnixStream::pair().unwrap();
        let server_task = tokio::spawn(async move {
            let req = read_request(&mut server, Duration::from_secs(1))
                .await
                .unwrap();
            matches!(req, Request::Status);
        });
        client.write_all(br#"{"type":"Status"}"#).await.unwrap();
        // Intentionally do not shutdown the client; server must still parse.
        server_task.await.unwrap();
    }
}
