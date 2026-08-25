pub(crate) mod config_api;
pub(crate) mod logs_api;
pub(crate) mod status_api;

/// Centralize IPC path resolution for GUI APIs.
pub fn socket_path() -> anyhow::Result<std::path::PathBuf> {
    Ok(backup_core::platform::paths::ipc_socket_path())
}
