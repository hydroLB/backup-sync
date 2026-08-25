use crate::config::model::HashingTuning;
use crate::hashing;
use anyhow::{Context, Result};
use std::path::Path;

/// Provide a stable content fingerprint with bounded IO.
pub fn hash_file_with_tuning(path: &Path, tuning: &HashingTuning) -> Result<String> {
    hashing::sha256_file_hex_with_tuning(path, tuning).with_context(|| {
        format!(
            "fs::hashing::hash_file_with_tuning failed computing sha256 for {:?}",
            path
        )
    })
}

/// Provide a safe default hashing behavior when tuning is unavailable.
pub fn hash_file(path: &Path) -> Result<String> {
    hash_file_with_tuning(path, &HashingTuning::default())
}
