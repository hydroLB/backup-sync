use serde::{Deserialize, Serialize};

/// Reduce destination IO and space usage while preserving plaintext-hash deduplication.
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
    /// Keep compression opt-in while making the config schema forward compatible.
    fn default() -> Self {
        Self {
            enabled: false,
            blob_chunk_bytes: crate::config::model::defaults::default_blob_compression_chunk_bytes(
            ),
            zstd_level: crate::config::model::defaults::default_blob_compression_zstd_level(),
        }
    }
}
