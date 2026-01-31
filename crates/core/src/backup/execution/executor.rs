use crate::backup::{naming, retention};
use crate::config::model::{ExecutionTuning, HashingTuning};
use crate::fs::hashing::hash_file_with_tuning;
use crate::logging::redact_path;
use crate::state::history;
use crate::state::models::{ActivityItem, FileState, StoredState};
use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};
use tracing::{info, span, warn, Level};

use super::{io, retry};

#[derive(Debug, Clone)]
pub struct BackupResult {
    pub backed_up: usize,
    pub errors: usize,
}

pub struct BackupExecutor {
    pub max_parallel_copies: usize,
    pub max_bytes_per_second: Option<u64>,
    pub min_free_space_bytes: Option<u64>,
    pub tuning: ExecutionTuning,
    pub hashing: HashingTuning,
}

impl BackupExecutor {
    /// Purpose: Builds an executor from the shared configuration model.
    ///
    /// Inputs: a reference to the shared config.
    /// Outputs: a ready to use `BackupExecutor` instance.
    /// Ties to: config defaults and runtime execution setup.
    /// Side effects: None.
    /// Why: keep executor wiring consistent across CLI, daemon, and GUI callers.
    pub fn from_config(cfg: &crate::config::model::Config) -> Self {
        Self {
            max_parallel_copies: cfg.max_parallel_copies,
            max_bytes_per_second: cfg.max_bytes_per_second,
            min_free_space_bytes: cfg.min_free_space_bytes,
            tuning: cfg.execution.clone(),
            hashing: cfg.hashing.clone(),
        }
    }

    /// Purpose: Executes the backup plan and aggregates per item results into a summary.
    ///
    /// Inputs: the planned items and the mutable state store.
    /// Outputs: a `BackupResult` with counts for UI and logging.
    /// Ties to: `backup_one` for per file execution and into state updates for error surfacing.
    /// Side effects: Reads and writes filesystem data, mutates stored state, and emits logs.
    /// Why: coordinate execution and keep state changes consistent across items.
    pub fn execute(
        &self,
        plan: &[crate::backup::planning::PlannedItem],
        state: &mut StoredState,
    ) -> Result<BackupResult> {
        let mut backed = 0usize;
        let mut errors = 0usize;
        let retry_delays = self.tuning.retry_delays();
        for item in plan {
            let span = span!(
                Level::INFO,
                "backup_file",
                path = %redact_path(&item.src),
                size = item.len,
                reason = %item.reason
            );
            let _guard = span.enter();
            match self
                .backup_one(item, state, &retry_delays)
                .with_context(|| format!("BackupExecutor::execute failed for {:?}", item.src))
            {
                Ok(_) => backed += 1,
                Err(e) => {
                    errors += 1;
                    let rendered = format!("{:#}", e);
                    state.last_error = Some(rendered.clone());
                    warn!("backup failed: {rendered}");
                }
            }
        }
        Ok(BackupResult {
            backed_up: backed,
            errors,
        })
    }

    /// Purpose: Performs a single item backup from source to destination and updates state.
    ///
    /// Inputs: the planned item, mutable state, and retry delays.
    /// Outputs: `Ok(())` when the backup is durable and state is updated.
    /// Ties to: `copy_with_throttle`, `retry_with_backoff`, and retention enforcement.
    /// Side effects: Reads and writes filesystem data and mutates stored state.
    /// Why: isolate per item work so failures do not break the entire cycle.
    fn backup_one(
        &self,
        item: &crate::backup::planning::PlannedItem,
        state: &mut StoredState,
        retry_delays: &[Duration],
    ) -> Result<()> {
        self.ensure_source_exists(&item.src)?;
        let hash = self.compute_hash(&item.src, &item.precomputed_hash, retry_delays)?;
        let (dir, final_path, tmp_path) = self.prepare_paths(item)?;
        self.ensure_free_space(&dir, item.len)?;
        self.write_temp_copy(&item.src, &tmp_path, retry_delays)?;
        self.verify_temp_size(&item.src, &tmp_path)?;
        self.finalize_copy(&tmp_path, &final_path)?;
        self.record_backup_state(item, hash, final_path, state)?;
        Ok(())
    }

    /// Purpose: Ensures the source path is present before attempting a backup.
    ///
    /// Inputs: the source path.
    /// Outputs: `Ok(())` when the source exists.
    /// Ties to: `backup_one` for upfront validation.
    /// Side effects: Reads filesystem metadata.
    /// Why: fail early with a specific error before doing IO work.
    fn ensure_source_exists(&self, src: &Path) -> Result<()> {
        if !src.exists() {
            anyhow::bail!(
                "BackupExecutor::ensure_source_exists source does not exist at backup time: {:?}",
                src
            );
        }
        Ok(())
    }

    /// Purpose: Computes the source hash, reusing a precomputed value when provided.
    ///
    /// Inputs: the source path, optional precomputed hash, and retry delays.
    /// Outputs: the resolved hash string.
    /// Ties to: `hash_file` and `retry_with_backoff` for resilience.
    /// Side effects: Reads file contents and may sleep between retries.
    /// Why: ensure state records the exact content hash for verification and change detection.
    fn compute_hash(
        &self,
        src: &Path,
        precomputed: &Option<String>,
        retry_delays: &[Duration],
    ) -> Result<String> {
        match precomputed {
            Some(h) => Ok(h.clone()),
            None => retry::retry_with_backoff("BackupExecutor::compute_hash", retry_delays, || {
                hash_file_with_tuning(src, &self.hashing).with_context(|| {
                    format!("BackupExecutor::compute_hash failed to hash {:?}", src)
                })
            }),
        }
    }

    /// Purpose: Builds destination paths and ensures the backup directory exists.
    ///
    /// Inputs: the planned item for destination root and source path.
    /// Outputs: the backup directory, final path, and temporary path.
    /// Ties to: naming helpers and filesystem directory creation.
    /// Side effects: Creates destination directories on disk.
    /// Why: keep file naming consistent and ensure a durable target path.
    fn prepare_paths(
        &self,
        item: &crate::backup::planning::PlannedItem,
    ) -> Result<(PathBuf, PathBuf, PathBuf)> {
        let dir = naming::backup_dir_for(&item.destination_root, &item.src);
        fs::create_dir_all(&dir).with_context(|| {
            format!(
                "BackupExecutor::prepare_paths failed to create backup directory {:?}",
                dir
            )
        })?;
        let filename = naming::backup_filename(&item.src);
        let final_path = dir.join(&filename);
        let tmp_path = dir.join(format!("{}.tmp", filename));
        Ok((dir, final_path, tmp_path))
    }

    /// Purpose: Checks destination free space against file size and configured thresholds.
    ///
    /// Inputs: the destination directory and source length.
    /// Outputs: `Ok(())` if free space is sufficient.
    /// Ties to: filesystem free space checks and config tuning.
    /// Side effects: Reads filesystem free space metadata.
    /// Why: avoid starting a copy that cannot complete.
    fn ensure_free_space(&self, dir: &Path, len: u64) -> Result<()> {
        let free = fs2::free_space(dir).with_context(|| {
            format!(
                "BackupExecutor::ensure_free_space failed to read free space for {:?}",
                dir
            )
        })?;
        let needed = len.saturating_add(self.tuning.free_space_safety_buffer_bytes);
        if needed > free {
            anyhow::bail!(
                "BackupExecutor::ensure_free_space insufficient space at {:?} (need {} bytes incl. safety buffer, have {})",
                dir,
                needed,
                free
            );
        }
        if let Some(min_free) = self.min_free_space_bytes {
            if free < min_free {
                anyhow::bail!(
                    "BackupExecutor::ensure_free_space free space {} below configured minimum {} at {:?}",
                    free,
                    min_free,
                    dir
                );
            }
        }
        Ok(())
    }

    /// Purpose: Copies the source file to a temporary path with retry and throttling.
    ///
    /// Inputs: the source path, temp path, retry delays, and timeout.
    /// Outputs: `Ok(())` once the temp file is fully written.
    /// Ties to: `copy_with_throttle` and `retry_with_backoff`.
    /// Side effects: Removes any existing temp file and writes new temp file data.
    /// Why: stage writes to allow verification before the final atomic move.
    fn write_temp_copy(
        &self,
        src: &Path,
        tmp_path: &Path,
        retry_delays: &[Duration],
    ) -> Result<()> {
        retry::retry_with_backoff("BackupExecutor::write_temp_copy", retry_delays, || {
            // Clean up previous temp if it exists before retry.
            let _ = fs::remove_file(tmp_path);
            io::copy_with_throttle(
                src,
                tmp_path,
                self.max_bytes_per_second,
                self.tuning.copy_buffer_bytes,
                Some(Duration::from_secs(self.tuning.copy_timeout_seconds)),
            )
            .with_context(|| {
                format!(
                    "BackupExecutor::write_temp_copy failed copying {:?} -> {:?}",
                    src, tmp_path
                )
            })
        })
    }

    /// Purpose: Verifies the temp file size matches the source before finalizing.
    ///
    /// Inputs: the source and temp paths.
    /// Outputs: `Ok(())` when sizes match.
    /// Ties to: filesystem metadata and temp copy staging.
    /// Side effects: Reads filesystem metadata and may delete the temp file on mismatch.
    /// Why: avoid committing a truncated or partial copy.
    fn verify_temp_size(&self, src: &Path, tmp_path: &Path) -> Result<()> {
        let src_meta = fs::metadata(src).with_context(|| {
            format!(
                "BackupExecutor::verify_temp_size failed to stat source {:?}",
                src
            )
        })?;
        let tmp_meta = fs::metadata(tmp_path).with_context(|| {
            format!(
                "BackupExecutor::verify_temp_size failed to stat temp backup {:?}",
                tmp_path
            )
        })?;
        if src_meta.len() != tmp_meta.len() {
            let _ = fs::remove_file(tmp_path);
            anyhow::bail!(
                "BackupExecutor::verify_temp_size backup size mismatch for {:?}",
                src
            );
        }
        Ok(())
    }

    /// Purpose: Atomically moves the temporary backup into its final location.
    ///
    /// Inputs: the temp path and final destination path.
    /// Outputs: `Ok(())` when the move succeeds.
    /// Ties to: filesystem rename semantics after verification.
    /// Side effects: Renames filesystem entries on disk.
    /// Why: ensure the final backup is a complete file with atomic visibility.
    fn finalize_copy(&self, tmp_path: &Path, final_path: &Path) -> Result<()> {
        fs::rename(tmp_path, final_path).with_context(|| {
            format!(
                "BackupExecutor::finalize_copy failed to move {:?} -> {:?}",
                tmp_path, final_path
            )
        })?;
        Ok(())
    }

    /// Purpose: Updates the stored state after a successful backup and applies retention.
    ///
    /// Inputs: the planned item, resolved hash, final path, and mutable state.
    /// Outputs: `Ok(())` after state has been updated.
    /// Ties to: retention enforcement and recent activity tracking.
    /// Side effects: Mutates stored state, deletes old backups per retention, and emits logs.
    /// Why: keep state, retention, and UI activity in sync with the filesystem.
    fn record_backup_state(
        &self,
        item: &crate::backup::planning::PlannedItem,
        hash: String,
        final_path: PathBuf,
        state: &mut StoredState,
    ) -> Result<()> {
        let entry = state
            .files
            .entry(item.key.clone())
            .or_insert_with(FileState::default);
        entry.len = item.len;
        entry.mtime = item.mtime;
        entry.last_hash = Some(hash);
        entry.stable_cycles = 0;
        entry.backups.push(final_path.clone());
        let retained = retention::enforce_on_disk(item.max_copies, entry.backups.clone())
            .with_context(|| {
                format!(
                    "BackupExecutor::record_backup_state failed enforcing retention for {:?}",
                    item.src
                )
            })?;
        entry.backups = retained;
        info!("backup complete -> {:?}", final_path);
        history::record_activity(
            state,
            ActivityItem {
                path: item.src.to_string_lossy().into_owned(),
                bytes: item.len,
                ts: chrono::Utc::now().timestamp(),
            },
            self.tuning.recent_activity_cap,
        );
        Ok(())
    }
}
