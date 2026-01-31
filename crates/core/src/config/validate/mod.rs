use crate::config::config_limits;
use crate::config::model::Config;
use anyhow::Result;

mod limits;
mod sections;

pub use limits::ValidationLimits;

/// Purpose: Validates configuration invariants before use by the daemon, CLI, or GUI.
///
/// Inputs: a reference to a `Config`.
/// Outputs: `Ok(())` when all invariants are met, otherwise a descriptive error.
/// Ties to: config loading, init flows, and runtime checks before a backup cycle runs.
/// Side effects: Reads filesystem metadata and may create destination directories for validation.
/// Why: fail fast with clear messages rather than running unsafe or invalid settings.
pub fn validate(cfg: &Config) -> Result<()> {
    let limits = config_limits();
    validate_with_limits(cfg, &limits)
}

/// Purpose: Validates configuration using an explicit set of limits.
///
/// Inputs: the config and validation limits.
/// Outputs: `Ok(())` when all invariants are met, otherwise a descriptive error.
/// Ties to: `validate` and advanced guardrail tuning.
/// Side effects: Reads filesystem metadata and may create destination directories for validation.
/// Why: allow callers and tests to adjust validation thresholds in one place.
pub fn validate_with_limits(cfg: &Config, limits: &ValidationLimits) -> Result<()> {
    sections::validate_with_limits(cfg, limits)
}
