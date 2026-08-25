use crate::config::load::default_config;
use crate::config::model::Config;
use crate::config::validate::ValidationLimits;
use anyhow::Result;

/// Keep config defaults and constraints in a single source of truth.
#[derive(Debug, Clone)]
pub struct ConfigRegistry {
    pub defaults: Config,
    pub limits: ValidationLimits,
}

/// Provide a centralized, reusable configuration registry.
pub fn config_registry() -> Result<ConfigRegistry> {
    let defaults = default_config()?;
    let limits = config_limits();
    Ok(ConfigRegistry { defaults, limits })
}

/// Centralize default config creation behind a single helper.
pub fn config_defaults() -> Result<Config> {
    default_config()
}

/// Keep validation limits centralized for consistent enforcement.
pub fn config_limits() -> ValidationLimits {
    ValidationLimits::default()
}
