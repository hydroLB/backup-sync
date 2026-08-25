use crate::config::model::Config;
use crate::config::validate::ValidationLimits;
use anyhow::{bail, Result};

/// Fail fast on invalid patterns to avoid silently skipping files.
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
