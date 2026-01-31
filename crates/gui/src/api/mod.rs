pub mod backup_api;
pub mod config_api;
pub mod logs_api;
pub mod status_api;

/// Purpose: Builds the IPC socket path for talking to the daemon.
///
/// Inputs: none.
/// Outputs: the filesystem path to the IPC socket.
/// Ties to: status and other IPC calls.
/// Side effects: None.
/// Why: centralize IPC path resolution for GUI APIs.
pub fn socket_path() -> anyhow::Result<std::path::PathBuf> {
    let mut p = dirs::runtime_dir().unwrap_or(std::env::temp_dir());
    p.push("backup_sync_ipc.sock");
    Ok(p)
}
