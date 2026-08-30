/// Keep a consistent, readable default in one place.
pub(crate) fn default_destination_id() -> String {
    "default".to_string()
}

/// Keep top-level scheduling defaults centralized.
pub(crate) fn default_interval_seconds() -> u64 {
    3600
}

/// Keep top-level retention defaults centralized.
pub(crate) fn default_max_backups_per_file() -> usize {
    4
}

/// Reduce surprise by skipping dotfiles unless explicitly enabled.
pub(crate) fn default_skip_hidden() -> bool {
    true
}

/// Skip noisy or build artifact paths by default.
pub(crate) fn default_ignore_patterns() -> Vec<String> {
    vec![
        "**/.DS_Store".into(),
        "**/node_modules/**".into(),
        "**/target/**".into(),
        "**/tmp/**".into(),
        "**/~$*".into(),
    ]
}

/// Keep encryption bounded in memory while remaining efficient on typical storage.
pub(crate) fn default_blob_encryption_chunk_bytes() -> usize {
    64 * 1024
}

/// Keep compression bounded in memory while remaining efficient on typical storage.
pub(crate) fn default_blob_compression_chunk_bytes() -> usize {
    64 * 1024
}

/// Provide a balanced default that trades CPU for better space savings.
pub(crate) fn default_blob_compression_zstd_level() -> i32 {
    3
}
