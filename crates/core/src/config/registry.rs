use crate::config::load::default_config;
use crate::config::model::Config;
use crate::config::validate::ValidationLimits;
use anyhow::Result;

/// Purpose: Central registry for configuration defaults and validation limits.
///
/// Inputs: None.
/// Outputs: A registry containing defaults and guardrail limits.
/// Ties to: Config loading, validation, and documentation.
/// Side effects: None.
/// Why: Keep config defaults and constraints in a single source of truth.
#[derive(Debug, Clone)]
pub struct ConfigRegistry {
    pub defaults: Config,
    pub limits: ValidationLimits,
}

/// Purpose: Build the config registry with defaults and limits.
///
/// Inputs: None.
/// Outputs: A `ConfigRegistry` instance.
/// Ties to: Config load paths and validation guardrails.
/// Side effects: Reads platform defaults when building config.
/// Why: Provide a centralized, reusable configuration registry.
pub fn config_registry() -> Result<ConfigRegistry> {
    let defaults = default_config()?;
    let limits = config_limits();
    Ok(ConfigRegistry { defaults, limits })
}

/// Purpose: Provide the default config in a reusable helper.
///
/// Inputs: None.
/// Outputs: A default `Config` value.
/// Ties to: CLI init, GUI bootstrapping, and diagnostics.
/// Side effects: Reads platform defaults when building config.
/// Why: Centralize default config creation behind a single helper.
pub fn config_defaults() -> Result<Config> {
    default_config()
}

/// Purpose: Provide default validation limits in a reusable helper.
///
/// Inputs: None.
/// Outputs: A `ValidationLimits` instance.
/// Ties to: Config validation guardrails.
/// Side effects: None.
/// Why: Keep validation limits centralized for consistent enforcement.
pub fn config_limits() -> ValidationLimits {
    ValidationLimits::default()
}
