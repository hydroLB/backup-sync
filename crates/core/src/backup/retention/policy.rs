use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use std::fs;
use std::path::PathBuf;

/// Purpose: Enforces retention by pruning oldest backup entries beyond the max.
///
/// Inputs: the maximum copies to keep and the existing backup paths.
/// Outputs: the retained list of backup paths.
/// Ties to: backup execution state updates and retention scheduling.
/// Side effects: None.
/// Why: keep per file history bounded while preserving the newest copies without requiring IO.
pub fn enforce(max: usize, mut existing: Vec<PathBuf>) -> Result<Vec<PathBuf>> {
    let (_pruned, retained) = partition(max, &mut existing)?;
    Ok(retained)
}

/// Purpose: Enforces retention and deletes pruned backups from disk.
///
/// Inputs: the maximum copies to keep and the existing backup paths.
/// Outputs: the retained list of backup paths.
/// Ties to: `enforce` for ordering and `BackupExecutor` retention enforcement.
/// Side effects: Deletes old backup files on disk.
/// Why: provide a durable retention boundary after new backups are recorded.
pub fn enforce_on_disk(max: usize, existing: Vec<PathBuf>) -> Result<Vec<PathBuf>> {
    let mut ordered = existing;
    let (pruned, retained) = partition(max, &mut ordered)?;
    for p in pruned {
        match fs::remove_file(&p) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(e).with_context(|| {
                    format!(
                        "retention::enforce_on_disk failed to remove backup file {:?}",
                        p
                    )
                });
            }
        }
    }
    Ok(retained)
}

/// Purpose: Parses a timestamp prefix from a backup filename for ordering.
///
/// Inputs: the path to a backup file.
/// Outputs: an optional Unix timestamp parsed from the filename prefix.
/// Ties to: retention sorting.
/// Side effects: None.
/// Why: use timestamp ordering to drop the oldest backups first.
fn parse_timestamp(p: &PathBuf) -> Option<i64> {
    let name = p.file_name()?.to_string_lossy();
    let ts = name.split("__").next()?;
    NaiveDateTime::parse_from_str(ts, "%Y%m%d-%H%M%S")
        .ok()
        .map(|dt| dt.and_utc().timestamp())
}

/// Purpose: Sorts backups by timestamp prefix and partitions into pruned and retained sets.
///
/// Inputs: max copies to keep and a mutable list of existing backup paths.
/// Outputs: `(pruned, retained)` where `retained.len() <= max`.
/// Ties to: `enforce` and `enforce_on_disk`.
/// Side effects: Reorders the provided vector in place.
/// Why: keep ordering and selection logic shared while keeping `enforce` IO-free.
fn partition(max: usize, existing: &mut Vec<PathBuf>) -> Result<(Vec<PathBuf>, Vec<PathBuf>)> {
    if max == 0 {
        anyhow::bail!("retention::partition max must be > 0");
    }
    if existing.len() <= max {
        return Ok((Vec::new(), std::mem::take(existing)));
    }
    existing.sort_by_key(|p| parse_timestamp(p).unwrap_or(i64::MAX));
    let keep_from = existing.len().saturating_sub(max);
    let retained = existing.split_off(keep_from);
    let pruned = std::mem::take(existing);
    Ok((pruned, retained))
}
