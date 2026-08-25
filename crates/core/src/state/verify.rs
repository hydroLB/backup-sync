use crate::config::model::HashingTuning;
use crate::fs::hashing::hash_file_with_tuning;
use crate::logging::redact_path;
use crate::state::models::StoredState;
use anyhow::Result;
use chrono::Utc;

/// Detect corruption or drift in the most recent backups.
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
