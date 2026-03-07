use std::path::PathBuf;

/// Summary: Holds daemon startup arguments sourced from the environment.
///
/// Inputs: environment variables.
///
/// Outputs: a structured argument set.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon initialization for config and log overrides.
///
/// Why this exists: allow non interactive overrides for service environments.
#[derive(Debug, Clone)]
pub struct DaemonArgs {
    pub config_path: Option<PathBuf>,
    pub log_path: Option<PathBuf>,
}

impl DaemonArgs {
    /// Summary: Loads daemon arguments from environment variables.
    ///
    /// Inputs: the current process environment.
    ///
    /// Outputs: a `DaemonArgs` instance.
    ///
    /// Side effects: None.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: daemon startup configuration.
    ///
    /// Why this exists: allow service managers to override paths without CLI flags.
    pub fn from_env() -> Self {
        let config_path = std::env::var_os("BACKUP_SYNC_CONFIG").map(PathBuf::from);
        let log_path = std::env::var_os("BACKUP_SYNC_LOG").map(PathBuf::from);
        Self {
            config_path,
            log_path,
        }
    }
}
