use crate::config::load::default_config;
use crate::config::model::Config;
use crate::config::validate::ValidationLimits;
use anyhow::Result;

/// Summary: Central registry for configuration defaults and validation limits.
///
/// Inputs: None.
///
/// Outputs: A registry containing defaults and guardrail limits.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Config loading, validation, and documentation.
///
/// Why this exists: Keep config defaults and constraints in a single source of truth.
#[derive(Debug, Clone)]
pub struct ConfigRegistry {
    pub defaults: Config,
    pub limits: ValidationLimits,
}

/// Summary: Build the config registry with defaults and limits.
///
/// Inputs: None.
///
/// Outputs: A `ConfigRegistry` instance.
///
/// Side effects: Reads platform defaults when building config.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Config load paths and validation guardrails.
///
/// Why this exists: Provide a centralized, reusable configuration registry.
pub fn config_registry() -> Result<ConfigRegistry> {
    let defaults = default_config()?;
    let limits = config_limits();
    Ok(ConfigRegistry { defaults, limits })
}

/// Summary: Provide the default config in a reusable helper.
///
/// Inputs: None.
///
/// Outputs: A default `Config` value.
///
/// Side effects: Reads platform defaults when building config.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: CLI init, GUI bootstrapping, and diagnostics.
///
/// Why this exists: Centralize default config creation behind a single helper.
pub fn config_defaults() -> Result<Config> {
    default_config()
}

/// Summary: Provide default validation limits in a reusable helper.
///
/// Inputs: None.
///
/// Outputs: A `ValidationLimits` instance.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Config validation guardrails.
///
/// Why this exists: Keep validation limits centralized for consistent enforcement.
pub fn config_limits() -> ValidationLimits {
    ValidationLimits::default()
}
