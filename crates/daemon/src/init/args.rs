use std::path::PathBuf;

/// Purpose: Holds daemon startup arguments sourced from the environment.
///
/// Inputs: environment variables.
/// Outputs: a structured argument set.
/// Ties to: daemon initialization for config and log overrides.
/// Side effects: None.
/// Why: allow non interactive overrides for service environments.
#[derive(Debug, Clone)]
pub struct DaemonArgs {
    pub config_path: Option<PathBuf>,
    pub log_path: Option<PathBuf>,
}

impl DaemonArgs {
    /// Purpose: Loads daemon arguments from environment variables.
    ///
    /// Inputs: the current process environment.
    /// Outputs: a `DaemonArgs` instance.
    /// Ties to: daemon startup configuration.
    /// Side effects: None.
    /// Why: allow service managers to override paths without CLI flags.
    pub fn from_env() -> Self {
        let config_path = std::env::var_os("BACKUP_SYNC_CONFIG").map(PathBuf::from);
        let log_path = std::env::var_os("BACKUP_SYNC_LOG").map(PathBuf::from);
        Self {
            config_path,
            log_path,
        }
    }
}
