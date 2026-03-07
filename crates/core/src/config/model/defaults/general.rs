/// Summary: Supplies the default destination id when one is omitted in config.
///
/// Inputs: none.
///
/// Outputs: a static default id string.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: config migration and validation for destination lookups.
///
/// Why this exists: keep a consistent, readable default in one place.
pub(crate) fn default_destination_id() -> String {
    "default".to_string()
}

/// Summary: Supplies the default backup interval in seconds.
///
/// Inputs: none.
///
/// Outputs: interval between scheduler cycles in seconds.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon interval scheduler cadence.
///
/// Why this exists: keep top-level scheduling defaults centralized.
pub(crate) fn default_interval_seconds() -> u64 {
    1800
}

/// Summary: Supplies the default per-file backup retention count.
///
/// Inputs: none.
///
/// Outputs: the default backup version cap per file.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: versioned retention pruning.
///
/// Why this exists: keep top-level retention defaults centralized.
pub(crate) fn default_max_backups_per_file() -> usize {
    5
}

/// Summary: Supplies the default skip hidden behavior for scanners.
///
/// Inputs: none.
///
/// Outputs: a boolean toggle for hidden entries.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: scan collection and config defaults.
///
/// Why this exists: reduce surprise by skipping dotfiles unless explicitly enabled.
pub(crate) fn default_skip_hidden() -> bool {
    true
}

/// Summary: Supplies the default ignore patterns for scan filters.
///
/// Inputs: none.
///
/// Outputs: a curated list of glob patterns.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `collect_targets` ignore matching.
///
/// Why this exists: skip noisy or build artifact paths by default.
pub(crate) fn default_ignore_patterns() -> Vec<String> {
    vec![
        "**/.DS_Store".into(),
        "**/node_modules/**".into(),
        "**/target/**".into(),
        "**/tmp/**".into(),
        "**/~$*".into(),
    ]
}

/// Summary: Supplies the default chunk size (in bytes) for encrypted blob streaming.
///
/// Inputs: none.
///
/// Outputs: the blob encryption chunk size in bytes.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: encrypted blob encoding and decoding.
///
/// Why this exists: keep encryption bounded in memory while remaining efficient on typical storage.
pub(crate) fn default_blob_encryption_chunk_bytes() -> usize {
    64 * 1024
}

/// Summary: Supplies the default chunk size (in bytes) for compressed blob streaming.
///
/// Inputs: none.
///
/// Outputs: the blob compression chunk size in bytes.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: compressed blob encoding and decoding.
///
/// Why this exists: keep compression bounded in memory while remaining efficient on typical storage.
pub(crate) fn default_blob_compression_chunk_bytes() -> usize {
    64 * 1024
}

/// Summary: Supplies the default Zstd compression level for blob writes.
///
/// Inputs: none.
///
/// Outputs: the zstd compression level as an integer.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: compressed blob encoding.
///
/// Why this exists: provide a balanced default that trades CPU for better space savings.
pub(crate) fn default_blob_compression_zstd_level() -> i32 {
    3
}
