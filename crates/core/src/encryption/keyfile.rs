use crate::config::model::Config;
use crate::hashing;
use crate::io::{run_with_policy, BlockingIoPolicy, CancellationFlag};
use crate::logging::redact_path;
use crate::platform::paths;
use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const KEY_MAGIC: &[u8; 8] = b"BSYNCKEY";
const KEY_VERSION: u8 = 1;
const KEY_LEN: usize = 32;

pub struct KeyInfo {
    pub key_id: String,
    key: [u8; KEY_LEN],
}

impl KeyInfo {
    pub(crate) fn key(&self) -> [u8; KEY_LEN] {
        self.key
    }
}

/// Keep key material in a predictable OS config location unless explicitly overridden.
pub fn resolve_key_path(cfg: &Config) -> Result<PathBuf> {
    if let Some(p) = cfg.encryption.key_path.clone() {
        return Ok(p);
    }
    paths::encryption_key_file_path()
        .context("encryption::keyfile::resolve_key_path failed to build default key path")
}

/// At-rest encryption is only recoverable with the same key; key creation must be explicit and auditable.
pub fn create_key_file(path: &Path, force: bool) -> Result<KeyInfo> {
    let io_policy = BlockingIoPolicy::bootstrap_defaults();
    let single_attempt = BlockingIoPolicy::single_attempt(io_policy.timeout);
    if path.exists() && !force {
        anyhow::bail!(
            "encryption::keyfile::create_key_file refusing to overwrite existing key file at {} (pass --force to overwrite)",
            redact_path(path)
        );
    }
    if let Some(parent) = path.parent() {
        run_with_policy(
            "encryption::keyfile::create_key_file create parent directory",
            &io_policy,
            CancellationFlag::none(),
            || {
                fs::create_dir_all(parent).with_context(|| {
                    format!(
                        "encryption::keyfile::create_key_file failed to create parent directory {}",
                        redact_path(parent)
                    )
                })
            },
        )?;
    }

    let mut key = [0u8; KEY_LEN];
    use chacha20poly1305::aead::rand_core::RngCore as _;
    chacha20poly1305::aead::rand_core::OsRng.fill_bytes(&mut key);
    let key_id = key_id(&key);

    let mut temp = tempfile::NamedTempFile::new_in(
        path.parent()
            .context("encryption::keyfile::create_key_file missing parent directory")?,
    )
    .context("encryption::keyfile::create_key_file failed to create temp file")?;

    run_with_policy(
        "encryption::keyfile::create_key_file write key payload",
        &single_attempt,
        CancellationFlag::none(),
        || {
            temp.write_all(KEY_MAGIC)
                .context("encryption::keyfile::create_key_file failed to write magic")?;
            temp.write_all(&[KEY_VERSION])
                .context("encryption::keyfile::create_key_file failed to write version")?;
            temp.write_all(&key)
                .context("encryption::keyfile::create_key_file failed to write key bytes")?;
            temp.flush()
                .context("encryption::keyfile::create_key_file failed to flush key file")?;
            temp.as_file()
                .sync_all()
                .context("encryption::keyfile::create_key_file failed to fsync key file")?;
            Ok(())
        },
    )?;

    #[cfg(target_family = "unix")]
    {
        use std::os::unix::fs::PermissionsExt;
        let perm = fs::Permissions::from_mode(0o600);
        temp.as_file()
            .set_permissions(perm)
            .context("encryption::keyfile::create_key_file failed to set permissions")?;
    }

    temp.persist(path).map_err(|e| {
        anyhow::anyhow!(
            "encryption::keyfile::create_key_file failed to persist {}: {}",
            redact_path(path),
            e
        )
    })?;

    Ok(KeyInfo { key_id, key })
}

/// Backups are only restorable with the original key; loading must be strict and explicit.
pub fn load_key_file(path: &Path) -> Result<KeyInfo> {
    let io_policy = BlockingIoPolicy::bootstrap_defaults();
    let raw = run_with_policy(
        "encryption::keyfile::load_key_file read key",
        &io_policy,
        CancellationFlag::none(),
        || {
            fs::read(path).with_context(|| {
                format!(
                    "encryption::keyfile::load_key_file failed to read {}",
                    redact_path(path)
                )
            })
        },
    )?;
    let min_len = KEY_MAGIC.len() + 1 + KEY_LEN;
    if raw.len() < min_len {
        anyhow::bail!(
            "encryption::keyfile::load_key_file invalid key file length at {} (expected >= {}, got {})",
            redact_path(path),
            min_len,
            raw.len()
        );
    }
    if &raw[..KEY_MAGIC.len()] != KEY_MAGIC {
        anyhow::bail!(
            "encryption::keyfile::load_key_file invalid key file magic at {}",
            redact_path(path)
        );
    }
    let ver = raw[KEY_MAGIC.len()];
    if ver != KEY_VERSION {
        anyhow::bail!(
            "encryption::keyfile::load_key_file unsupported key file version {} at {}",
            ver,
            redact_path(path)
        );
    }
    let start = KEY_MAGIC.len() + 1;
    let end = start + KEY_LEN;
    let mut key = [0u8; KEY_LEN];
    key.copy_from_slice(&raw[start..end]);
    Ok(KeyInfo {
        key_id: key_id(&key),
        key,
    })
}

fn key_id(key: &[u8; KEY_LEN]) -> String {
    hashing::sha256_hex(key)
}
