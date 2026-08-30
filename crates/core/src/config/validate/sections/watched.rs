use crate::config::model::Config;
use crate::config::validate::ValidationLimits;
use anyhow::{bail, Result};
use std::collections::HashSet;

use super::destinations::DestinationIndex;

/// Allow users to configure destinations and schedules before selecting folders.
pub(crate) fn validate_presence(_cfg: &Config, _label: &str) -> Result<()> {
    Ok(())
}

/// Cap scan and plan work to keep cycles bounded.
pub(crate) fn validate_count(cfg: &Config, limits: &ValidationLimits, label: &str) -> Result<()> {
    if cfg.watched.len() > limits.max_watched {
        bail!(
            "{label} too many watched entries (>{}); trim the list to keep the daemon responsive",
            limits.max_watched
        );
    }
    Ok(())
}

/// Prevent recursive backups and duplicate watched entries that are hard to debug.
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

    // The same source may intentionally be protected to multiple destinations. Only the
    // source/destination pair must be unique; rejecting the path globally breaks redundancy.
    let mut seen = HashSet::new();
    for w in &cfg.watched {
        if w.path.parent().is_none() {
            bail!(
                "{label} watched path cannot be filesystem root: {:?}",
                w.path
            );
        }
        let key = (w.path.clone(), w.destination_id.clone());
        if !seen.insert(key) {
            bail!(
                "{label} duplicate watched path/destination pair: {:?} -> {}",
                w.path,
                w.destination_id
            );
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
