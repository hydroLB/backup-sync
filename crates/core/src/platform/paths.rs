use anyhow::Result;
use dirs::config_dir;
use std::path::PathBuf;

/// Keep config storage in the OS config directory.
pub fn config_file_path() -> Result<PathBuf> {
    let mut p = config_dir()
        .ok_or_else(|| anyhow::anyhow!("platform::paths::config_file_path no config dir"))?;
    p.push("backup_sync/config.toml");
    Ok(p)
}

/// Keep state storage alongside config in the OS config directory.
pub fn state_file_path() -> Result<PathBuf> {
    let mut p = config_dir()
        .ok_or_else(|| anyhow::anyhow!("platform::paths::state_file_path no config dir"))?;
    p.push("backup_sync/state.json");
    Ok(p)
}

/// Place backups under the OS data directory by default.
pub fn default_backup_root() -> Result<PathBuf> {
    let mut p = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("platform::paths::default_backup_root no data dir"))?;
    p.push("backup_sync/backups");
    Ok(p)
}

/// Keep logs under the OS data directory.
pub fn log_file_path() -> Result<PathBuf> {
    let mut p = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("platform::paths::log_file_path no data dir"))?;
    p.push("backup_sync/logs/daemon.log");
    Ok(p)
}

/// Keep key material out of the destination store so backups remain recoverable only with the user key.
pub fn encryption_key_file_path() -> Result<PathBuf> {
    let mut p = config_dir().ok_or_else(|| {
        anyhow::anyhow!("platform::paths::encryption_key_file_path no config dir")
    })?;
    p.push("backup_sync/key_v1.bin");
    Ok(p)
}

/// Resolve the per-user Unix-domain socket used by the daemon and its clients.
///
/// Tests and isolated development sessions can override the location with
/// `BACKUP_SYNC_IPC_SOCKET`. The production default lives in a user-specific
/// runtime directory so one account cannot collide with another.
#[cfg(unix)]
pub fn ipc_socket_path() -> PathBuf {
    if let Some(path) = std::env::var_os("BACKUP_SYNC_IPC_SOCKET") {
        return PathBuf::from(path);
    }

    let mut path = dirs::runtime_dir().unwrap_or_else(std::env::temp_dir);
    path.push(format!("backup-sync-{}", effective_user_id()));
    path.push("backup_sync_ipc.sock");
    path
}

#[cfg(not(unix))]
pub fn ipc_socket_path() -> PathBuf {
    if let Some(path) = std::env::var_os("BACKUP_SYNC_IPC_SOCKET") {
        return PathBuf::from(path);
    }

    let mut path = dirs::runtime_dir().unwrap_or_else(std::env::temp_dir);
    path.push("backup-sync/backup_sync_ipc.sock");
    path
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

#[cfg(all(test, unix))]
mod tests {
    use super::ipc_socket_path;

    #[test]
    fn ipc_socket_uses_a_user_scoped_runtime_directory() {
        if std::env::var_os("BACKUP_SYNC_IPC_SOCKET").is_some() {
            return;
        }

        let path = ipc_socket_path();
        let parent = path.parent().expect("socket must have a parent");
        assert!(parent
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("backup-sync-")));
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("backup_sync_ipc.sock")
        );
    }
}
