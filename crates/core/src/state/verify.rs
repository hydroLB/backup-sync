use crate::config::model::HashingTuning;
use crate::fs::hashing::hash_file_with_tuning;
use crate::logging::redact_path;
use crate::state::models::StoredState;
use anyhow::Result;
use chrono::Utc;

/// Summary: Re-hashes the most recent backup for each tracked file and records verification results.
///
/// Inputs: mutable state and hashing tuning values.
///
/// Outputs: a tuple of (ok_count, issue_count).
///
/// Side effects: Reads backup files, mutates stored state, and emits warnings.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon verification cycles and UI status reporting.
///
/// Why this exists: detect corruption or drift in the most recent backups.
pub fn verify_backups(state: &mut StoredState, hashing: &HashingTuning) -> Result<(usize, usize)> {
    let mut ok = 0usize;
    let mut bad = 0usize;
    for (path, meta) in state.files.iter() {
        if let Some(last) = meta.backups.last() {
            match hash_file_with_tuning(last, hashing) {
                Ok(h) => {
                    if Some(&h) == meta.last_hash.as_ref() {
                        ok += 1;
                    } else {
                        bad += 1;
                        tracing::warn!(
                            "state::verify::verify_backups hash mismatch for {} -> {}",
                            redact_path(std::path::Path::new(path)),
                            redact_path(last)
                        );
                    }
                }
                Err(e) => {
                    bad += 1;
                    tracing::warn!(
                        "state::verify::verify_backups failed to hash {:?} for {}: {}",
                        redact_path(last),
                        redact_path(std::path::Path::new(path)),
                        e
                    );
                }
            }
        }
    }
    state.last_verify_ts = Some(Utc::now().timestamp());
    state.last_verify_issues = Some(bad);
    state.last_verify_status = Some(if bad == 0 {
        "ok".into()
    } else {
        "issues_detected".into()
    });
    Ok((ok, bad))
}
