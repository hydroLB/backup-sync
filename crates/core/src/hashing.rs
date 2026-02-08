use crate::config::model::HashingTuning;
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::time::{Duration, Instant};

/// Purpose: Canonical content hash algorithm used across the repo.
///
/// Inputs: None.
/// Outputs: A static algorithm label.
/// Side effects: None.
/// Error handling: None.
/// Ties to other methods: Used by the versioned blob store, restore verification, and integrity scrubs.
/// Why this exists: Standardize hashing choices so manifests, verification, and IO planning stay consistent.
pub const CONTENT_HASH_ALGORITHM: &str = "sha256";

/// Purpose: Hex-encode bytes using lowercase characters.
///
/// Inputs: raw bytes.
/// Outputs: a lowercase hex string.
/// Side effects: None.
/// Error handling: Never fails.
/// Ties to other methods: Used by `sha256_hex` and callers formatting SHA-256 digests.
/// Why this exists: Keep hex formatting consistent across subsystems.
pub fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{:02x}", b);
    }
    out
}

/// Purpose: Compute a SHA-256 digest of in-memory bytes and return a lowercase hex string.
///
/// Inputs: input bytes.
/// Outputs: SHA-256 digest as lowercase hex.
/// Side effects: None.
/// Error handling: Never fails.
/// Ties to other methods: Used for stable IDs (source ids) and key identifiers.
/// Why this exists: Keep all SHA-256 digests formatted consistently.
pub fn sha256_hex(input: &[u8]) -> String {
    let digest = Sha256::digest(input);
    hex_lower(digest.as_slice())
}

/// Purpose: Compute a SHA-256 hash for a file using explicit hashing tuning.
///
/// Inputs: file path and hashing tuning values.
/// Outputs: hex-encoded SHA-256 string for the file contents.
/// Side effects: Reads file contents from disk.
/// Error handling: Returns contextual errors for open/read failures and timeouts.
/// Ties to other methods: Used by versioned manifest scanning and legacy verification routines.
/// Why this exists: Centralize file hashing so all subsystems share the same algorithm and tuning behavior.
pub fn sha256_file_hex_with_tuning(path: &Path, tuning: &HashingTuning) -> Result<String> {
    if tuning.buffer_bytes == 0 {
        anyhow::bail!("hashing::sha256_file_hex_with_tuning buffer_bytes must be > 0");
    }
    if tuning.timeout_seconds == 0 {
        anyhow::bail!("hashing::sha256_file_hex_with_tuning timeout_seconds must be > 0");
    }
    let f = File::open(path).with_context(|| {
        format!(
            "hashing::sha256_file_hex_with_tuning failed to open file for hashing: {:?}",
            path
        )
    })?;
    let mut reader = BufReader::with_capacity(tuning.buffer_bytes, f);
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; tuning.buffer_bytes];
    let timeout = Duration::from_secs(tuning.timeout_seconds);
    let start = Instant::now();
    loop {
        if start.elapsed() > timeout {
            anyhow::bail!(
                "hashing::sha256_file_hex_with_tuning timed out after {}s for {:?}",
                tuning.timeout_seconds,
                path
            );
        }
        let n = reader.read(&mut buf).with_context(|| {
            format!(
                "hashing::sha256_file_hex_with_tuning failed to read file while hashing: {:?}",
                path
            )
        })?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex_lower(hasher.finalize().as_slice()))
}

/// Purpose: Compute a SHA-256 hash for a file using default hashing tuning.
///
/// Inputs: file path.
/// Outputs: hex-encoded SHA-256 string.
/// Side effects: Reads file contents from disk.
/// Error handling: Propagates hashing errors and timeouts.
/// Ties to other methods: Used by perf guard and benches.
/// Why this exists: Provide a safe default for callers without access to config tuning.
pub fn sha256_file_hex(path: &Path) -> Result<String> {
    sha256_file_hex_with_tuning(path, &HashingTuning::default())
}
