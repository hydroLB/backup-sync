use std::path::PathBuf;

/// Allow non interactive overrides for service environments.
#[derive(Debug, Clone)]
pub struct DaemonArgs {
    pub config_path: Option<PathBuf>,
    pub log_path: Option<PathBuf>,
}

impl DaemonArgs {
    /// Allow service managers to override paths without CLI flags.
    pub fn from_env() -> Self {
        let config_path = std::env::var_os("BACKUP_SYNC_CONFIG").map(PathBuf::from);
        let log_path = std::env::var_os("BACKUP_SYNC_LOG").map(PathBuf::from);
        Self {
            config_path,
            log_path,
        }
    }
}
