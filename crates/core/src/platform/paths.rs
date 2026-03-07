use anyhow::Result;
use dirs::config_dir;
use std::path::PathBuf;

/// Summary: Builds the default config file path.
///
/// Inputs: none.
///
/// Outputs: the filesystem path to the config file.
///
/// Side effects: Reads OS configuration directories.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: config load and save operations.
///
/// Why this exists: keep config storage in the OS config directory.
pub fn config_file_path() -> Result<PathBuf> {
    let mut p = config_dir()
        .ok_or_else(|| anyhow::anyhow!("platform::paths::config_file_path no config dir"))?;
    p.push("backup_sync/config.toml");
    Ok(p)
}

/// Summary: Builds the default state file path.
///
/// Inputs: none.
///
/// Outputs: the filesystem path to the state file.
///
/// Side effects: Reads OS configuration directories.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: state load and save operations.
///
/// Why this exists: keep state storage alongside config in the OS config directory.
pub fn state_file_path() -> Result<PathBuf> {
    let mut p = config_dir()
        .ok_or_else(|| anyhow::anyhow!("platform::paths::state_file_path no config dir"))?;
    p.push("backup_sync/state.json");
    Ok(p)
}

/// Summary: Builds the default backup root directory path.
///
/// Inputs: none.
///
/// Outputs: the filesystem path to the backup root.
///
/// Side effects: Reads OS data directory locations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: default config creation.
///
/// Why this exists: place backups under the OS data directory by default.
pub fn default_backup_root() -> Result<PathBuf> {
    let mut p = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("platform::paths::default_backup_root no data dir"))?;
    p.push("backup_sync/backups");
    Ok(p)
}

/// Summary: Builds the default log file path for the daemon.
///
/// Inputs: none.
///
/// Outputs: the filesystem path to the daemon log file.
///
/// Side effects: Reads OS data directory locations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: logging initialization.
///
/// Why this exists: keep logs under the OS data directory.
pub fn log_file_path() -> Result<PathBuf> {
    let mut p = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("platform::paths::log_file_path no data dir"))?;
    p.push("backup_sync/logs/daemon.log");
    Ok(p)
}

/// Summary: Builds the default encryption key file path for at-rest blob encryption.
///
/// Inputs: none.
///
/// Outputs: the filesystem path to the encryption key file.
///
/// Side effects: Reads OS configuration directories.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: encryption config defaults and keyfile loading.
///
/// Why this exists: keep key material out of the destination store so backups remain recoverable only with the user key.
pub fn encryption_key_file_path() -> Result<PathBuf> {
    let mut p = config_dir().ok_or_else(|| {
        anyhow::anyhow!("platform::paths::encryption_key_file_path no config dir")
    })?;
    p.push("backup_sync/key_v1.bin");
    Ok(p)
}
