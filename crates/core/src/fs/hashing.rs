use crate::config::model::HashingTuning;
use crate::hashing;
use anyhow::{Context, Result};
use std::path::Path;

/// Summary: Computes a SHA-256 hash for a file using explicit hashing tuning.
///
/// Inputs: the filesystem path and hashing tuning values.
///
/// Outputs: a hex encoded hash string.
///
/// Side effects: Reads file contents and system time for timeout enforcement.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: change detection and verification workflows.
///
/// Why this exists: provide a stable content fingerprint with bounded IO.
pub fn hash_file_with_tuning(path: &Path, tuning: &HashingTuning) -> Result<String> {
    hashing::sha256_file_hex_with_tuning(path, tuning).with_context(|| {
        format!(
            "fs::hashing::hash_file_with_tuning failed computing sha256 for {:?}",
            path
        )
    })
}

/// Summary: Computes a SHA-256 hash for a file using default hashing tuning.
///
/// Inputs: the filesystem path to the file.
///
/// Outputs: a hex encoded hash string.
///
/// Side effects: Reads file contents and system time for timeout enforcement.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: hashing call sites without explicit tuning context.
///
/// Why this exists: provide a safe default hashing behavior when tuning is unavailable.
pub fn hash_file(path: &Path) -> Result<String> {
    hash_file_with_tuning(path, &HashingTuning::default())
}
