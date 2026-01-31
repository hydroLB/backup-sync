use crate::config::model::HashingTuning;
use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::time::{Duration, Instant};

/// Purpose: Computes a BLAKE3 hash for a file using explicit hashing tuning.
///
/// Inputs: the filesystem path and hashing tuning values.
/// Outputs: a hex encoded hash string.
/// Ties to: change detection and verification workflows.
/// Side effects: Reads file contents and system time for timeout enforcement.
/// Why: provide a stable content fingerprint with bounded IO.
pub fn hash_file_with_tuning(path: &Path, tuning: &HashingTuning) -> Result<String> {
    if tuning.buffer_bytes == 0 {
        anyhow::bail!("fs::hashing::hash_file_with_tuning buffer_bytes must be > 0");
    }
    if tuning.timeout_seconds == 0 {
        anyhow::bail!("fs::hashing::hash_file_with_tuning timeout_seconds must be > 0");
    }
    let f = File::open(path).with_context(|| {
        format!(
            "fs::hashing::hash_file_with_tuning failed to open file for hashing: {:?}",
            path
        )
    })?;
    let mut reader = BufReader::with_capacity(tuning.buffer_bytes, f);
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; tuning.buffer_bytes];
    let timeout = Duration::from_secs(tuning.timeout_seconds);
    let start = Instant::now();
    loop {
        if start.elapsed() > timeout {
            anyhow::bail!(
                "fs::hashing::hash_file_with_tuning timed out after {}s for {:?}",
                tuning.timeout_seconds,
                path
            );
        }
        let n = reader.read(&mut buf).with_context(|| {
            format!(
                "fs::hashing::hash_file_with_tuning failed to read file while hashing: {:?}",
                path
            )
        })?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

/// Purpose: Computes a BLAKE3 hash for a file using default hashing tuning.
///
/// Inputs: the filesystem path to the file.
/// Outputs: a hex encoded hash string.
/// Ties to: hashing call sites without explicit tuning context.
/// Side effects: Reads file contents and system time for timeout enforcement.
/// Why: provide a safe default hashing behavior when tuning is unavailable.
pub fn hash_file(path: &Path) -> Result<String> {
    hash_file_with_tuning(path, &HashingTuning::default())
}
