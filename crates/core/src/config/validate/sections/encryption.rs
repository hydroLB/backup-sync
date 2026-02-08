use crate::config::model::Config;
use crate::config::validate::ValidationLimits;
use crate::encryption::keyfile;
use crate::logging::redact_path;
use anyhow::{bail, Context, Result};

/// Purpose: Validates encryption configuration and key material prerequisites.
///
/// Inputs: the config, validation limits, and a label prefix.
/// Outputs: `Ok(())` when encryption is disabled or when enabled prerequisites are satisfied.
/// Ties to: versioned blob encryption for write/restore/scrub.
/// Side effects: Reads key files from disk when encryption is enabled.
/// Why: fail fast with actionable errors before starting daemon cycles.
pub(crate) fn validate_encryption(
    cfg: &Config,
    limits: &ValidationLimits,
    label: &str,
) -> Result<()> {
    if !cfg.encryption.enabled {
        return Ok(());
    }

    if cfg.encryption.blob_chunk_bytes == 0 {
        bail!("{label} encryption.blob_chunk_bytes must be > 0");
    }
    if cfg.encryption.blob_chunk_bytes > limits.max_blob_encryption_chunk_bytes {
        bail!(
            "{label} encryption.blob_chunk_bytes too large (>{} bytes)",
            limits.max_blob_encryption_chunk_bytes
        );
    }

    let key_path = keyfile::resolve_key_path(cfg)
        .context("config::validate_encryption failed to resolve encryption key path")?;
    if !key_path.exists() {
        bail!(
            "{label} encryption enabled but key file does not exist at {}. Create one with `cargo run -p cli -- keygen` and set encryption.key_id.",
            redact_path(&key_path)
        );
    }
    if !key_path.is_file() {
        bail!(
            "{label} encryption enabled but key path is not a file: {}",
            redact_path(&key_path)
        );
    }

    let expected = cfg.encryption.key_id.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "{label} encryption enabled but encryption.key_id is missing. Run `cargo run -p cli -- keygen` and copy the printed key_id into config."
        )
    })?;

    let info = keyfile::load_key_file(&key_path).with_context(|| {
        format!(
            "config::validate_encryption failed to load key file {}",
            redact_path(&key_path)
        )
    })?;
    if info.key_id != expected {
        bail!(
            "{label} encryption key_id mismatch for {} (expected {}, got {}). Refusing to proceed to avoid irreversible writes with the wrong key.",
            redact_path(&key_path),
            expected,
            info.key_id
        );
    }

    Ok(())
}
