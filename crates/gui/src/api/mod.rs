pub(crate) mod config_api;
pub(crate) mod logs_api;
pub(crate) mod status_api;

/// Summary: Builds the IPC socket path for talking to the daemon.
///
/// Inputs: none.
///
/// Outputs: the filesystem path to the IPC socket.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: status and other IPC calls.
///
/// Why this exists: centralize IPC path resolution for GUI APIs.
pub fn socket_path() -> anyhow::Result<std::path::PathBuf> {
    let mut p = dirs::runtime_dir().unwrap_or(std::env::temp_dir());
    p.push("backup_sync_ipc.sock");
    Ok(p)
}
