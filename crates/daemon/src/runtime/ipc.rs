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
}

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
                    let mut buf = Vec::new();
                    match timeout(ipc_timeout, stream.read_to_end(&mut buf)).await {
                        Ok(Ok(_)) => {}
                        Ok(Err(e)) => {
                            tracing::error!("daemon::runtime::ipc spawn_server read error: {e:?}");
                            continue;
                        }
                        Err(_) => {
                            tracing::error!(
                                "daemon::runtime::ipc spawn_server read timeout after {:?}",
                                ipc_timeout
                            );
                            continue;
                        }
                    }
                    let req = match serde_json::from_slice::<Request>(&buf) {
                        Ok(r) => r,
                        Err(e) => {
                            tracing::error!("daemon::runtime::ipc spawn_server parse error: {e:?}");
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
                    let mut buf = Vec::new();
                    match timeout(ipc_timeout, server.read_to_end(&mut buf)).await {
                        Ok(Ok(_)) => {
                            if let Ok(req) = serde_json::from_slice::<Request>(&buf) {
                                match req {
                                    Request::Status => {
                                        let st = state.lock().await.clone();
                                        let reply = build_status_reply(
                                            &st,
                                            &destinations,
                                            recent_activity_limit,
                                        );
                                        match timeout(
                                            ipc_timeout,
                                            write_status_reply(&mut server, &reply),
                                        )
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
                                }
                            }
                        }
                        Ok(Err(e)) => {
                            tracing::error!("daemon::runtime::ipc spawn_server read error: {e:?}");
                        }
                        Err(_) => {
                            tracing::error!(
                                "daemon::runtime::ipc spawn_server read timeout after {:?}",
                                ipc_timeout
                            );
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
