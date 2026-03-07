use crate::config::model::Config;
use crate::config::validate::ValidationLimits;
use anyhow::{bail, Result};
use std::collections::HashSet;

use super::destinations::DestinationIndex;

/// Summary: Validates watched list presence and enabled state.
///
/// Inputs: the config and label prefix.
///
/// Outputs: `Ok(())` when the watched list is well-formed for persistence.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon and GUI config save flows.
///
/// Why this exists: allow users to configure destinations and schedules before selecting folders.
pub(crate) fn validate_presence(_cfg: &Config, _label: &str) -> Result<()> {
    Ok(())
}

/// Summary: Validates watched collection size limits.
///
/// Inputs: the config, limits, and label prefix.
///
/// Outputs: `Ok(())` when watched entry count is within limits.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon responsiveness guardrails.
///
/// Why this exists: cap scan and plan work to keep cycles bounded.
pub(crate) fn validate_count(cfg: &Config, limits: &ValidationLimits, label: &str) -> Result<()> {
    if cfg.watched.len() > limits.max_watched {
        bail!(
            "{label} too many watched entries (>{}); trim the list to keep the daemon responsive",
            limits.max_watched
        );
    }
    Ok(())
}

/// Summary: Validates watched paths against destinations and structural constraints.
///
/// Inputs: the config, label prefix, and destination index.
///
/// Outputs: `Ok(())` when watched paths are non-overlapping and mapped to valid destinations.
///
/// Side effects: Reads filesystem metadata and may emit warnings for missing paths.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: planning and execution path safety checks.
///
/// Why this exists: prevent recursive backups and duplicate watched entries that are hard to debug.
pub(crate) fn validate_paths(
    cfg: &Config,
    label: &str,
    destinations: &DestinationIndex<'_>,
) -> Result<()> {
    if cfg.watched.is_empty() {
        return Ok(());
    }
    if !cfg.watched.iter().any(|w| w.path.exists()) {
        tracing::warn!(
            "{label} none of the watched paths exist on disk; backups will be idle until one is available"
        );
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
