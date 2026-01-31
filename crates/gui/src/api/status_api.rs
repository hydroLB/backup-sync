use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

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
    pub uptime_secs: Option<i64>,
    pub version: Option<String>,
    pub free_bytes: Option<u64>,
    pub last_verify_ts: Option<i64>,
    pub last_verify_status: Option<String>,
    pub last_verify_issues: Option<usize>,
    pub recent_activity: Vec<backup_core::ActivityItem>,
    pub safe_mode: bool,
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
    pub free_bytes: Option<u64>,
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
    let mut stream = UnixStream::connect(&socket)
        .await
        .with_context(|| format!("status_api::fetch_status failed to connect to {:?}", socket))?;
    let req = serde_json::json!({ "type": "Status" });
    stream
        .write_all(req.to_string().as_bytes())
        .await
        .context("status_api::fetch_status failed to write status request")?;
    let mut buf = Vec::new();
    stream
        .read_to_end(&mut buf)
        .await
        .context("status_api::fetch_status failed to read status response")?;
    if buf.is_empty() {
        return Err(anyhow!(
            "status_api::fetch_status empty status response from daemon"
        ));
    }
    let raw = String::from_utf8_lossy(&buf);
    Ok(serde_json::from_slice(&buf)
        .with_context(|| format!("status_api::fetch_status failed to parse response: {}", raw))?)
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
    let mut stream = ClientOptions::new()
        .open(r"\\.\pipe\backup_sync_ipc")
        .with_context(|| {
            "status_api::fetch_status failed to connect to named pipe \\\\.\\pipe\\backup_sync_ipc"
        })?;
    let req = serde_json::json!({ "type": "Status" });
    stream
        .write_all(req.to_string().as_bytes())
        .await
        .context("status_api::fetch_status failed to write status request")?;
    let mut buf = Vec::new();
    stream
        .read_to_end(&mut buf)
        .await
        .context("status_api::fetch_status failed to read status response")?;
    if buf.is_empty() {
        return Err(anyhow!(
            "status_api::fetch_status empty status response from daemon"
        ));
    }
    let raw = String::from_utf8_lossy(&buf);
    Ok(serde_json::from_slice(&buf)
        .with_context(|| format!("status_api::fetch_status failed to parse response: {}", raw))?)
}
