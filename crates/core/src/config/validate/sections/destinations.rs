use crate::config::model::{Config, Destination};
use anyhow::{bail, Result};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use tracing::warn;

#[derive(Debug)]
pub(crate) struct DestinationIndex<'a> {
    by_id: HashMap<&'a str, &'a Path>,
}

impl<'a> DestinationIndex<'a> {
    /// Purpose: Resolves a destination by id.
    ///
    /// Inputs: the destination id string.
    /// Outputs: the destination path when present.
    /// Ties to: watched path validation and overlap checks.
    /// Side effects: None.
    /// Why: keep destination id lookup centralized and type safe.
    pub(crate) fn get(&self, id: &str) -> Option<&'a Path> {
        self.by_id.get(id).copied()
    }
}

/// Purpose: Validates destinations and builds a lookup index by destination id.
///
/// Inputs: the config and label prefix.
/// Outputs: a `DestinationIndex` for downstream watched-path checks.
/// Ties to: destination selection in planning and runtime execution.
/// Side effects: May create destination directories on disk during validation.
/// Why: ensure destination id mapping is sane and writable before running a cycle.
pub(crate) fn validate_and_index<'cfg>(
    cfg: &'cfg Config,
    label: &str,
) -> Result<DestinationIndex<'cfg>> {
    // Validate destinations (for now we expect at least one; multi-dest support can expand later).
    if cfg.destinations.is_empty() {
        bail!("{label} no destinations configured; add at least one destination");
    }
    let mut ids = HashSet::new();
    let mut by_id: HashMap<&str, &Path> = HashMap::new();
    for d in &cfg.destinations {
        validate_one_destination(cfg, d, label, &mut ids)?;
        by_id.insert(d.id.as_str(), d.path.as_path());
    }
    validate_replication_targets(cfg, label, &by_id)?;
    Ok(DestinationIndex { by_id })
}

/// Purpose: Validates a single destination entry.
///
/// Inputs: the config, destination, label prefix, and an id set.
/// Outputs: `Ok(())` when the destination passes structural and filesystem checks.
/// Ties to: `validate_and_index`.
/// Side effects: May create the destination directory on disk.
/// Why: ensure each destination is usable before any cycle starts.
fn validate_one_destination(
    cfg: &Config,
    d: &Destination,
    label: &str,
    ids: &mut HashSet<String>,
) -> Result<()> {
    if d.id.trim().is_empty() {
        bail!("{label} destination id cannot be empty");
    }
    if !ids.insert(d.id.clone()) {
        bail!("{label} duplicate destination id: {}", d.id);
    }
    if d.max_backups_per_file.unwrap_or(cfg.max_backups_per_file) == 0 {
        bail!(
            "{label} destination {} max_backups_per_file must be > 0",
            d.id
        );
    }
    if d.path.as_os_str().is_empty() {
        bail!("{label} destination {} has empty path", d.id);
    }
    if let Some(parent) = d.path.parent() {
        if !parent.exists() {
            warn!(
                "{label} destination {} parent does not exist yet (drive disconnected?): {:?}",
                d.id, parent
            );
        }
    } else {
        bail!(
            "{label} destination {} cannot be filesystem root; choose a folder inside your home directory.",
            d.id
        );
    }
    if d.path.exists() && d.path.is_file() {
        bail!(
            "{label} destination {} points to a file; choose a folder instead: {:?}",
            d.id,
            d.path
        );
    }
    if d.path.exists() {
        if let Err(e) = fs::create_dir_all(&d.path) {
            bail!(
                "{label} destination {} is not writable or creatable at {:?}: {}",
                d.id,
                d.path,
                e
            );
        }
    } else {
        warn!(
            "{label} destination {} path does not exist yet (drive disconnected?): {:?}",
            d.id, d.path
        );
    }
    Ok(())
}

/// Purpose: Validate destination replication topology (replicate_to ids).
///
/// Inputs: loaded config, label prefix, and a destination id index.
/// Outputs: `Ok(())` when all replicate_to references are valid.
/// Ties to: versioned store replication and 3-2-1 workflows.
/// Side effects: None.
/// Why: replication references must be validated early to avoid silent no-op replication.
fn validate_replication_targets(
    cfg: &Config,
    label: &str,
    by_id: &HashMap<&str, &Path>,
) -> Result<()> {
    for d in &cfg.destinations {
        for target in d.replicate_to.iter() {
            if target.trim().is_empty() {
                bail!(
                    "{label} destination {} replicate_to cannot contain empty ids",
                    d.id
                );
            }
            if target == &d.id {
                bail!(
                    "{label} destination {} replicate_to cannot include itself",
                    d.id
                );
            }
            if !by_id.contains_key(target.as_str()) {
                bail!(
                    "{label} destination {} replicate_to references unknown destination id {}",
                    d.id,
                    target
                );
            }
        }
    }
    Ok(())
}
