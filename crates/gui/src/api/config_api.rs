use anyhow::{Context, Result};
use backup_core::{config::model::Config, load_config, save_config as core_save_config, validate};

/// Purpose: Loads the current configuration from disk.
///
/// Inputs: none.
/// Outputs: the loaded config.
/// Ties to: GUI config commands.
/// Side effects: Reads the config file from disk.
/// Why: provide configuration access for the frontend.
pub fn get_config() -> Result<Config> {
    load_config().context("gui::api::config_api::get_config failed to load config")
}

/// Purpose: Validates and saves a configuration update.
///
/// Inputs: the config to persist.
/// Outputs: `Ok(())` when validation and save succeed.
/// Ties to: GUI config save flows.
/// Side effects: Writes the config file to disk.
/// Why: ensure config changes are validated before writing.
pub fn save_config(cfg: &Config) -> Result<()> {
    validate(cfg).context("gui::api::config_api::save_config config validation failed")?;
    core_save_config(cfg).context("gui::api::config_api::save_config failed to save config")
}
