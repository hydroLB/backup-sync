use crate::io::{run_with_policy, BlockingIoPolicy, CancellationFlag};
use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use std::fs;
use std::path::{Path, PathBuf};

/// Summary: Enforces retention by pruning oldest backup entries beyond the max.
///
/// Inputs: the maximum copies to keep and the existing backup paths.
///
/// Outputs: the retained list of backup paths.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: backup execution state updates and retention scheduling.
///
/// Why this exists: keep per file history bounded while preserving the newest copies without requiring IO.
pub fn enforce(max: usize, mut existing: Vec<PathBuf>) -> Result<Vec<PathBuf>> {
    let (_pruned, retained) = partition(max, &mut existing)?;
    Ok(retained)
}

/// Summary: Enforces retention and deletes pruned backups from disk.
///
/// Inputs: the maximum copies to keep and the existing backup paths.
///
/// Outputs: the retained list of backup paths.
///
/// Side effects: Deletes old backup files on disk.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `enforce` for ordering and `BackupExecutor` retention enforcement.
///
/// Why this exists: provide a durable retention boundary after new backups are recorded.
pub fn enforce_on_disk(max: usize, existing: Vec<PathBuf>) -> Result<Vec<PathBuf>> {
    let io_policy = BlockingIoPolicy::bootstrap_defaults();
    let mut ordered = existing;
    let (pruned, retained) = partition(max, &mut ordered)?;
    for p in pruned {
        match run_with_policy(
            "backup::retention::enforce_on_disk remove pruned backup file",
            &io_policy,
            CancellationFlag::none(),
            || {
                fs::remove_file(&p).map_err(|error| {
                    anyhow::anyhow!(error)
                        .context("backup::retention::enforce_on_disk failed removing backup file")
                })
            },
        ) {
            Ok(()) => {}
            Err(e) => {
                if e.downcast_ref::<std::io::Error>()
                    .is_some_and(|io_error| io_error.kind() == std::io::ErrorKind::NotFound)
                {
                    continue;
                }
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

/// Summary: Parses a timestamp prefix from a backup filename for ordering.
///
/// Inputs: the path to a backup file.
///
/// Outputs: an optional Unix timestamp parsed from the filename prefix.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: retention sorting.
///
/// Why this exists: use timestamp ordering to drop the oldest backups first.
fn parse_timestamp(p: &Path) -> Option<i64> {
    let name = p.file_name()?.to_string_lossy();
    let ts = name.split("__").next()?;
    match NaiveDateTime::parse_from_str(ts, "%Y%m%d-%H%M%S") {
        Ok(dt) => Some(dt.and_utc().timestamp()),
        Err(_) => None,
    }
}

/// Summary: Sorts backups by timestamp prefix and partitions into pruned and retained sets.
///
/// Inputs: max copies to keep and a mutable list of existing backup paths.
///
/// Outputs: `(pruned, retained)` where `retained.len() <= max`.
///
/// Side effects: Reorders the provided vector in place.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `enforce` and `enforce_on_disk`.
///
/// Why this exists: keep ordering and selection logic shared while keeping `enforce` IO-free.
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
