use crate::config::model::Config;
use crate::config::validate::ValidationLimits;
use anyhow::{bail, Result};

/// Summary: Validates ignore pattern syntax and collection size.
///
/// Inputs: the config, limits, and label prefix.
///
/// Outputs: `Ok(())` when patterns compile and list size is within limits.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: scan filtering during planning and verification.
///
/// Why this exists: fail fast on invalid patterns to avoid silently skipping files.
pub(crate) fn validate_ignore_patterns(
    cfg: &Config,
    limits: &ValidationLimits,
    label: &str,
) -> Result<()> {
    for pat in &cfg.ignore_patterns {
        if globset::Glob::new(pat).is_err() {
            bail!("{label} invalid ignore pattern: {}", pat);
        }
    }
    if cfg.ignore_patterns.len() > limits.max_ignore_patterns {
        bail!(
            "{label} too many ignore patterns (>{}); reduce the list",
            limits.max_ignore_patterns
        );
    }
    Ok(())
}
