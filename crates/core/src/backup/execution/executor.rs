use crate::backup::{naming, retention};
use crate::config::model::{ExecutionTuning, HashingTuning};
use crate::fs::hashing::hash_file_with_tuning;
use crate::io::{run_with_policy, BlockingIoPolicy, CancellationFlag};
use crate::logging::redact_path;
use crate::state::history;
use crate::state::models::{ActivityItem, StoredState};
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
    /// Keep executor wiring consistent across CLI, daemon, and GUI callers.
    pub fn from_config(cfg: &crate::config::model::Config) -> Self {
        Self {
            max_parallel_copies: cfg.max_parallel_copies,
            max_bytes_per_second: cfg.max_bytes_per_second,
            min_free_space_bytes: cfg.min_free_space_bytes,
            tuning: cfg.execution.clone(),
            hashing: cfg.hashing.clone(),
        }
    }

    /// Coordinate execution and keep state changes consistent across items.
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

    /// Isolate per item work so failures do not break the entire cycle.
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

    /// Fail early with a specific error before doing IO work.
    fn ensure_source_exists(&self, src: &Path) -> Result<()> {
        if !src.exists() {
            anyhow::bail!(
                "BackupExecutor::ensure_source_exists source does not exist at backup time: {:?}",
                src
            );
        }
        Ok(())
    }

    /// Ensure state records the exact content hash for verification and change detection.
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

    /// Keep file naming consistent and ensure a durable target path.
    fn prepare_paths(
        &self,
        item: &crate::backup::planning::PlannedItem,
    ) -> Result<(PathBuf, PathBuf, PathBuf)> {
        let dir = naming::backup_dir_for(&item.destination_root, &item.src);
        let io_policy = BlockingIoPolicy::from_execution_tuning(&self.tuning);
        run_with_policy(
            "backup::execution::BackupExecutor::prepare_paths create backup directory",
            &io_policy,
            CancellationFlag::none(),
            || {
                fs::create_dir_all(&dir).with_context(|| {
                    format!(
                        "BackupExecutor::prepare_paths failed to create backup directory {:?}",
                        dir
                    )
                })
            },
        )?;
        let filename = naming::backup_filename(&item.src);
        let final_path = dir.join(&filename);
        let tmp_path = dir.join(format!("{}.tmp", filename));
        Ok((dir, final_path, tmp_path))
    }

    /// Avoid starting a copy that cannot complete.
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

    /// Stage writes to allow verification before the final atomic move.
    fn write_temp_copy(
        &self,
        src: &Path,
        tmp_path: &Path,
        retry_delays: &[Duration],
    ) -> Result<()> {
        retry::retry_with_backoff("BackupExecutor::write_temp_copy", retry_delays, || {
            // Clean up previous temp if it exists before retry.
            if let Err(error) = run_with_policy(
                "backup::execution::BackupExecutor::write_temp_copy remove stale temp file",
                &BlockingIoPolicy::single_attempt(Duration::from_secs(
                    self.tuning.copy_timeout_seconds.max(1),
                )),
                CancellationFlag::none(),
                || {
                    fs::remove_file(tmp_path).map_err(|io_error| {
                        anyhow::anyhow!(io_error)
                            .context("BackupExecutor::write_temp_copy failed removing stale temp")
                    })
                },
            ) {
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_none_or(|io_error| io_error.kind() != std::io::ErrorKind::NotFound)
                {
                    warn!(
                        path = %redact_path(tmp_path),
                        error = %error,
                        "BackupExecutor::write_temp_copy failed cleaning stale temp file"
                    );
                }
            }
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

    /// Avoid committing a truncated or partial copy.
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
            if let Err(error) = run_with_policy(
                "backup::execution::BackupExecutor::verify_temp_size remove mismatched temp file",
                &BlockingIoPolicy::single_attempt(Duration::from_secs(
                    self.tuning.copy_timeout_seconds.max(1),
                )),
                CancellationFlag::none(),
                || {
                    fs::remove_file(tmp_path).map_err(|io_error| {
                        anyhow::anyhow!(io_error).context(
                            "BackupExecutor::verify_temp_size failed removing mismatched temp",
                        )
                    })
                },
            ) {
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_none_or(|io_error| io_error.kind() != std::io::ErrorKind::NotFound)
                {
                    warn!(
                        path = %redact_path(tmp_path),
                        error = %error,
                        "BackupExecutor::verify_temp_size failed removing mismatched temp file"
                    );
                }
            }
            anyhow::bail!(
                "BackupExecutor::verify_temp_size backup size mismatch for {:?}",
                src
            );
        }
        Ok(())
    }

    /// Ensure the final backup is a complete file with atomic visibility.
    fn finalize_copy(&self, tmp_path: &Path, final_path: &Path) -> Result<()> {
        fs::rename(tmp_path, final_path).with_context(|| {
            format!(
                "BackupExecutor::finalize_copy failed to move {:?} -> {:?}",
                tmp_path, final_path
            )
        })?;
        Ok(())
    }

    /// Keep state, retention, and UI activity in sync with the filesystem.
    fn record_backup_state(
        &self,
        item: &crate::backup::planning::PlannedItem,
        hash: String,
        final_path: PathBuf,
        state: &mut StoredState,
    ) -> Result<()> {
        let entry = state.files.entry(item.key.clone()).or_default();
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
