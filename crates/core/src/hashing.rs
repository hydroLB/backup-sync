use crate::config::model::HashingTuning;
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::time::{Duration, Instant};

/// Standardize hashing choices so manifests, verification, and IO planning stay consistent.
pub const CONTENT_HASH_ALGORITHM: &str = "sha256";

/// Keep hex formatting consistent across subsystems.
pub fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        write!(out, "{:02x}", b)
            .expect("hashing::hex_lower failed writing to in-memory String buffer");
    }
    out
}

/// Keep all SHA-256 digests formatted consistently.
pub fn sha256_hex(input: &[u8]) -> String {
    let digest = Sha256::digest(input);
    hex_lower(digest.as_slice())
}

/// Centralize file hashing so all subsystems share the same algorithm and tuning behavior.
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

/// Provide a safe default for callers without access to config tuning.
pub fn sha256_file_hex(path: &Path) -> Result<String> {
    sha256_file_hex_with_tuning(path, &HashingTuning::default())
}
