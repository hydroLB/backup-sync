use crate::config::model::Config;
use crate::config::validate::ValidationLimits;
use anyhow::{bail, Result};
use std::collections::HashSet;

use super::destinations::DestinationIndex;

/// Purpose: Validates watched list presence and enabled state.
///
/// Inputs: the config and label prefix.
/// Outputs: `Ok(())` when at least one watched path is present and enabled.
/// Ties to: daemon and CLI startup flows.
/// Side effects: None.
/// Why: avoid running backup cycles with no work configured.
pub(crate) fn validate_presence(cfg: &Config, label: &str) -> Result<()> {
    if cfg.watched.is_empty() {
        bail!(
            "{label} no watched paths configured; run `backup-sync init` or add a folder in the app"
        );
    }
    if !cfg.watched.iter().any(|w| w.enabled) {
        bail!("{label} all watched paths are disabled; enable at least one to run backups");
    }
    Ok(())
}

/// Purpose: Validates watched collection size limits.
///
/// Inputs: the config, limits, and label prefix.
/// Outputs: `Ok(())` when watched entry count is within limits.
/// Ties to: daemon responsiveness guardrails.
/// Side effects: None.
/// Why: cap scan and plan work to keep cycles bounded.
pub(crate) fn validate_count(cfg: &Config, limits: &ValidationLimits, label: &str) -> Result<()> {
    if cfg.watched.len() > limits.max_watched {
        bail!(
            "{label} too many watched entries (>{}); trim the list to keep the daemon responsive",
            limits.max_watched
        );
    }
    Ok(())
}

/// Purpose: Validates watched paths against destinations and structural constraints.
///
/// Inputs: the config, label prefix, and destination index.
/// Outputs: `Ok(())` when watched paths are non-overlapping and mapped to valid destinations.
/// Ties to: planning and execution path safety checks.
/// Side effects: Reads filesystem metadata and may emit warnings for missing paths.
/// Why: prevent recursive backups and duplicate watched entries that are hard to debug.
pub(crate) fn validate_paths(
    cfg: &Config,
    label: &str,
    destinations: &DestinationIndex<'_>,
) -> Result<()> {
    if !cfg.watched.iter().any(|w| w.path.exists()) {
        bail!("{label} none of the watched paths exist on disk; add an existing folder or file");
    }

    // prevent recursive/overlapping paths and duplicates
    let mut seen = HashSet::new();
    for w in &cfg.watched {
        if w.path.parent().is_none() {
            bail!(
                "{label} watched path cannot be filesystem root: {:?}",
                w.path
            );
        }
        let key = w.path.to_string_lossy().to_string();
        if !seen.insert(key) {
            bail!("{label} duplicate watched path: {:?}", w.path);
        }
        let dest_path = destinations.get(w.destination_id.as_str()).ok_or_else(|| {
            anyhow::anyhow!(
                "{label} watched path missing destination {}",
                w.destination_id
            )
        })?;
        if w.path.starts_with(dest_path) {
            bail!(
                "{label} watched path {:?} is inside destination {:?}; choose a destination outside watched folders",
                w.path,
                dest_path
            );
        }
        if dest_path.starts_with(&w.path) {
            bail!(
                "{label} destination {:?} is inside a watched path {:?}; choose a destination outside watched folders",
                dest_path,
                w.path
            );
        }
        if !w.path.exists() {
            tracing::warn!("{label} watched path does not exist: {:?}", w.path);
        }
    }
    Ok(())
}
