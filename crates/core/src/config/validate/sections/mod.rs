use crate::config::model::Config;
use crate::config::validate::ValidationLimits;
use anyhow::{bail, Result};

mod compression;
mod destinations;
mod encryption;
mod ignore_patterns;
mod tuning;
mod watched;

/// Purpose: Validates configuration using an explicit set of limits.
///
/// Inputs: the config and validation limits.
/// Outputs: `Ok(())` when all invariants are met, otherwise a descriptive error.
/// Ties to: `config::validate::validate_with_limits`.
/// Side effects: Reads filesystem metadata and may create destination directories for validation.
/// Why: keep the top-level validation readable by delegating checks to focused modules.
pub(crate) fn validate_with_limits(cfg: &Config, limits: &ValidationLimits) -> Result<()> {
    let label = "config::validate_with_limits";
    watched::validate_presence(cfg, label)?;
    tuning::validate_tuning(cfg, limits, label)?;
    compression::validate_compression(cfg, limits, label)?;
    encryption::validate_encryption(cfg, limits, label)?;
    ignore_patterns::validate_ignore_patterns(cfg, limits, label)?;
    watched::validate_count(cfg, limits, label)?;
    let destinations = destinations::validate_and_index(cfg, label)?;
    watched::validate_paths(cfg, label, &destinations)?;
    Ok(())
}

/// Purpose: Ensures a numeric value is set to a positive integer.
///
/// Inputs: the label, field name, and the value.
/// Outputs: `Ok(())` if the value is greater than zero.
/// Ties to: multiple per-field guardrails.
/// Side effects: None.
/// Why: keep repetitive numeric validation consistent and readable.
fn ensure_nonzero_u64(label: &str, field: &str, value: u64) -> Result<()> {
    if value == 0 {
        bail!("{label} {field} must be > 0");
    }
    Ok(())
}

/// Purpose: Ensures a numeric value is set to a positive integer.
///
/// Inputs: the label, field name, and the value.
/// Outputs: `Ok(())` if the value is greater than zero.
/// Ties to: multiple per-field guardrails.
/// Side effects: None.
/// Why: keep repetitive numeric validation consistent and readable.
fn ensure_nonzero_usize(label: &str, field: &str, value: usize) -> Result<()> {
    if value == 0 {
        bail!("{label} {field} must be > 0");
    }
    Ok(())
}
