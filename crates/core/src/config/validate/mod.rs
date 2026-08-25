use crate::config::config_limits;
use crate::config::model::Config;
use anyhow::Result;

mod limits;
mod sections;

pub use limits::ValidationLimits;

/// Fail fast with clear messages rather than running unsafe or invalid settings.
pub fn validate(cfg: &Config) -> Result<()> {
    let limits = config_limits();
    validate_with_limits(cfg, &limits)
}

/// Allow callers and tests to adjust validation thresholds in one place.
pub fn validate_with_limits(cfg: &Config, limits: &ValidationLimits) -> Result<()> {
    sections::validate_with_limits(cfg, limits)
}
