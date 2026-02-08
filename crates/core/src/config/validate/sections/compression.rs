use crate::config::model::Config;
use crate::config::validate::ValidationLimits;
use anyhow::{bail, Result};

/// Purpose: Validates blob compression configuration.
///
/// Inputs: the config, validation limits, and a label prefix.
/// Outputs: `Ok(())` when compression is disabled or when enabled fields are within guardrails.
/// Ties to: versioned blob compression for write paths.
/// Side effects: None.
/// Why: prevent surprising memory usage or invalid codec settings.
pub(crate) fn validate_compression(
    cfg: &Config,
    limits: &ValidationLimits,
    label: &str,
) -> Result<()> {
    if !cfg.compression.enabled {
        return Ok(());
    }

    if cfg.compression.blob_chunk_bytes == 0 {
        bail!("{label} compression.blob_chunk_bytes must be > 0");
    }
    if cfg.compression.blob_chunk_bytes > limits.max_blob_compression_chunk_bytes {
        bail!(
            "{label} compression.blob_chunk_bytes too large (>{} bytes)",
            limits.max_blob_compression_chunk_bytes
        );
    }
    if cfg.compression.zstd_level < limits.min_blob_compression_zstd_level
        || cfg.compression.zstd_level > limits.max_blob_compression_zstd_level
    {
        bail!(
            "{label} compression.zstd_level must be between {} and {}",
            limits.min_blob_compression_zstd_level,
            limits.max_blob_compression_zstd_level
        );
    }

    Ok(())
}
