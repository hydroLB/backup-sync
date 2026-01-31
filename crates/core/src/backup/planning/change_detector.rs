use crate::config::model::{HashingTuning, PlanningTuning};
use crate::fs::{hashing::hash_file_with_tuning, scanning::metadata::FileMeta};
use crate::logging::redact_path;
use crate::state::models::StoredState;
use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing::{debug, warn};

#[derive(Debug, Clone)]
pub struct PlannedItem {
    pub src: PathBuf,
    pub len: u64,
    pub mtime: i64,
    pub reason: String,
    pub precomputed_hash: Option<String>,
    pub key: String,
    pub destination_root: PathBuf,
    pub max_copies: usize,
}

pub type BackupPlan = Vec<PlannedItem>;

/// Purpose: Builds a backup plan by comparing current filesystem metadata to stored state.
///
/// Inputs: the collected file metadata, mutable state, planning tuning, and hashing tuning.
/// Outputs: a list of planned items to back up.
/// Ties to: `hash_file` for periodic content verification and into `StoredState` updates.
/// Side effects: Reads file contents for hashing and mutates stored state.
/// Why: surface only changed or untracked files while limiting expensive hash checks.
pub fn plan(
    files: Vec<FileMeta>,
    state: &mut StoredState,
    tuning: &PlanningTuning,
    hashing: &HashingTuning,
) -> Result<BackupPlan> {
    let mut plan = Vec::with_capacity(files.len());
    for meta in files {
        let mut reason = None;
        let mut pre_hash = None;

        match state.files.get_mut(&meta.key) {
            None => reason = Some("new_file"),
            Some(s) => {
                if s.len != meta.len {
                    reason = Some("size_changed");
                    s.stable_cycles = 0;
                } else if s.mtime != meta.mtime {
                    reason = Some("timestamp_changed");
                    s.stable_cycles = 0;
                } else {
                    // Metadata unchanged; only hash every N stable cycles to reduce IO.
                    let should_hash = s.last_hash.is_some()
                        && (s.stable_cycles % tuning.hash_check_interval == 0);
                    if should_hash {
                        match hash_file_with_tuning(&meta.path, hashing).with_context(|| {
                            format!(
                                "planning::plan hash check failed for {}",
                                redact_path(&meta.path)
                            )
                        }) {
                            Ok(h) => {
                                if let Some(last_hash) = s.last_hash.as_ref() {
                                    if &h != last_hash {
                                        reason = Some("content_hash_changed");
                                        pre_hash = Some(h);
                                        s.stable_cycles = 0;
                                    } else {
                                        s.stable_cycles = s.stable_cycles.saturating_add(1);
                                    }
                                } else {
                                    warn!(
                                        "planning::plan missing stored hash for {}; backing up",
                                        redact_path(&meta.path)
                                    );
                                    reason = Some("missing_hash_state");
                                    pre_hash = Some(h);
                                    s.stable_cycles = 0;
                                }
                            }
                            Err(e) => {
                                warn!(
                                    "hash check failed for {}: {}; backing up just in case",
                                    redact_path(&meta.path),
                                    e
                                );
                                reason = Some("hash_check_failed");
                                s.stable_cycles = 0;
                            }
                        }
                    } else {
                        s.stable_cycles = s.stable_cycles.saturating_add(1);
                    }
                }
            }
        }

        if let Some(r) = reason {
            debug!(
                "planning backup for {} (reason: {})",
                redact_path(&meta.path),
                r
            );
            plan.push(PlannedItem {
                src: meta.path,
                len: meta.len,
                mtime: meta.mtime,
                reason: r.into(),
                precomputed_hash: pre_hash,
                key: meta.key,
                destination_root: meta.destination_root,
                max_copies: meta.max_copies,
            });
        }
    }
    Ok(plan)
}
