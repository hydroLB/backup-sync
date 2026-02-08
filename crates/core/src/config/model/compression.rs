use serde::{Deserialize, Serialize};

/// Summary: Configuration for blob compression at rest.
///
/// Inputs: Loaded from the config file under `[compression]`.
/// Outputs: Controls whether new blobs are written compressed and which codec/settings are used.
/// Side effects: None.
/// Error handling: Validated by `config::validate` when enabled.
/// Ties to other methods: Used by versioned blob writes and blob decoding for restore/scrub.
/// Why this exists: Reduce destination IO and space usage while preserving plaintext-hash deduplication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "crate::config::model::defaults::default_blob_compression_chunk_bytes")]
    pub blob_chunk_bytes: usize,
    #[serde(default = "crate::config::model::defaults::default_blob_compression_zstd_level")]
    pub zstd_level: i32,
}

impl Default for CompressionConfig {
    /// Summary: Build the default compression configuration.
    ///
    /// Inputs: None.
    /// Outputs: A disabled compression config with safe defaults.
    /// Side effects: None.
    /// Error handling: None.
    /// Ties to other methods: Used by config defaults and serde `default` behavior.
    /// Why this exists: Keep compression opt-in while making the config schema forward compatible.
    fn default() -> Self {
        Self {
            enabled: false,
            blob_chunk_bytes: crate::config::model::defaults::default_blob_compression_chunk_bytes(
            ),
            zstd_level: crate::config::model::defaults::default_blob_compression_zstd_level(),
        }
    }
}
