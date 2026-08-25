use anyhow::{Context, Result};
use backup_core::{
    config::model::Config, load_validated_config, save_config as core_save_config, validate,
};

/// Provide configuration access for the frontend.
pub fn get_config() -> Result<Config> {
    load_validated_config().context("gui::api::config_api::get_config failed to load config")
}

/// Ensure config changes are validated before writing.
pub fn save_config(cfg: &Config) -> Result<()> {
    validate(cfg).context("gui::api::config_api::save_config config validation failed")?;
    core_save_config(cfg).context("gui::api::config_api::save_config failed to save config")
}
