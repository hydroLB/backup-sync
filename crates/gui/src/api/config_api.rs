use anyhow::{Context, Result};
use backup_core::{
    config::model::Config, load_validated_config, save_config as core_save_config, validate,
};

/// Summary: Loads the current configuration from disk.
///
/// Inputs: none.
///
/// Outputs: the loaded config.
///
/// Side effects: Reads the config file from disk.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI config commands.
///
/// Why this exists: provide configuration access for the frontend.
pub fn get_config() -> Result<Config> {
    load_validated_config().context("gui::api::config_api::get_config failed to load config")
}

/// Summary: Validates and saves a configuration update.
///
/// Inputs: the config to persist.
///
/// Outputs: `Ok(())` when validation and save succeed.
///
/// Side effects: Writes the config file to disk.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI config save flows.
///
/// Why this exists: ensure config changes are validated before writing.
pub fn save_config(cfg: &Config) -> Result<()> {
    validate(cfg).context("gui::api::config_api::save_config config validation failed")?;
    core_save_config(cfg).context("gui::api::config_api::save_config failed to save config")
}
