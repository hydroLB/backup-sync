/// Purpose: Supplies the default destination id when one is omitted in config.
///
/// Inputs: none.
/// Outputs: a static default id string.
/// Ties to: config migration and validation for destination lookups.
/// Side effects: None.
/// Why: keep a consistent, readable default in one place.
pub(crate) fn default_destination_id() -> String {
    "default".to_string()
}

/// Purpose: Supplies the default skip hidden behavior for scanners.
///
/// Inputs: none.
/// Outputs: a boolean toggle for hidden entries.
/// Ties to: scan collection and config defaults.
/// Side effects: None.
/// Why: reduce surprise by skipping dotfiles unless explicitly enabled.
pub(crate) fn default_skip_hidden() -> bool {
    true
}

/// Purpose: Supplies the default ignore patterns for scan filters.
///
/// Inputs: none.
/// Outputs: a curated list of glob patterns.
/// Ties to: `collect_targets` ignore matching.
/// Side effects: None.
/// Why: skip noisy or build artifact paths by default.
pub(crate) fn default_ignore_patterns() -> Vec<String> {
    vec![
        "**/.DS_Store".into(),
        "**/node_modules/**".into(),
        "**/target/**".into(),
        "**/tmp/**".into(),
        "**/~$*".into(),
    ]
}

/// Purpose: Supplies the default chunk size (in bytes) for encrypted blob streaming.
///
/// Inputs: none.
/// Outputs: the blob encryption chunk size in bytes.
/// Ties to: encrypted blob encoding and decoding.
/// Side effects: None.
/// Why: keep encryption bounded in memory while remaining efficient on typical storage.
pub(crate) fn default_blob_encryption_chunk_bytes() -> usize {
    64 * 1024
}

/// Purpose: Supplies the default chunk size (in bytes) for compressed blob streaming.
///
/// Inputs: none.
/// Outputs: the blob compression chunk size in bytes.
/// Ties to: compressed blob encoding and decoding.
/// Side effects: None.
/// Why: keep compression bounded in memory while remaining efficient on typical storage.
pub(crate) fn default_blob_compression_chunk_bytes() -> usize {
    64 * 1024
}

/// Purpose: Supplies the default Zstd compression level for blob writes.
///
/// Inputs: none.
/// Outputs: the zstd compression level as an integer.
/// Ties to: compressed blob encoding.
/// Side effects: None.
/// Why: provide a balanced default that trades CPU for better space savings.
pub(crate) fn default_blob_compression_zstd_level() -> i32 {
    3
}
