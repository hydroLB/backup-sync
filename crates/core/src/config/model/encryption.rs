use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Provide opt-in confidentiality for stored blob bytes without changing the manifest schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub key_path: Option<PathBuf>,
    #[serde(default)]
    pub key_id: Option<String>,
    #[serde(default = "crate::config::model::defaults::default_blob_encryption_chunk_bytes")]
    pub blob_chunk_bytes: usize,
}

impl Default for EncryptionConfig {
    /// Keep encryption opt-in while making the config schema forward compatible.
    fn default() -> Self {
        Self {
            enabled: false,
            key_path: None,
            key_id: None,
            blob_chunk_bytes: crate::config::model::defaults::default_blob_encryption_chunk_bytes(),
        }
    }
}
