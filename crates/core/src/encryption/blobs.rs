use crate::config::model::Config;
use crate::encryption::keyfile;
use crate::logging::redact_path;
use anyhow::{Context, Result};
use chacha20poly1305::aead::stream::{
    DecryptorBE32, EncryptorBE32, Nonce as StreamNonce, StreamBE32,
};
use chacha20poly1305::aead::KeyInit;
use chacha20poly1305::{Key, XChaCha20Poly1305};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};

const ENC_MAGIC: &[u8; 8] = b"BSYNCENC";
const ENC_VERSION: u8 = 1;
const ALG_XCHACHA20POLY1305_BE32: u8 = 1;
const ALG_XCHACHA20POLY1305_BE32_ZSTD_CHUNKED: u8 = 2;
const XCHACHA20POLY1305_STREAM_NONCE_LEN: usize = 19;

const COMP_MAGIC: &[u8; 8] = b"BSYNCCMP";
const COMP_VERSION: u8 = 1;
const COMP_ALG_ZSTD_CHUNKED: u8 = 1;

const MAX_DECOMPRESSED_CHUNK_BYTES: usize = 8 * 1024 * 1024;

pub struct BlobCipher {
    key: [u8; 32],
    chunk_bytes: usize,
}

pub struct BlobCodec {
    cipher: Option<BlobCipher>,
    write_compression_enabled: bool,
    write_compression_chunk_bytes: usize,
    write_compression_zstd_level: i32,
}

impl BlobCodec {
    /// Summary: Build a reusable blob codec from the provided config.
    ///
    /// Inputs: Loaded config.
    ///
    /// Outputs: A `BlobCodec` that can write and decode blobs according to config.
    ///
    /// Side effects: Loads encryption key material when encryption is enabled.
    ///
    /// Error handling: Returns contextual errors for key loading and config inconsistencies.
    ///
    /// Ties to other methods: Used by versioned backup write, restore, and scrub routines.
    ///
    /// Why this exists: Avoid reloading key files and re-parsing config for each blob operation.
    pub fn from_config(cfg: &Config) -> Result<Self> {
        let cipher = BlobCipher::from_config(cfg)?;
        Ok(Self {
            cipher,
            write_compression_enabled: cfg.compression.enabled,
            write_compression_chunk_bytes: cfg.compression.blob_chunk_bytes.max(1),
            write_compression_zstd_level: cfg.compression.zstd_level,
        })
    }

    /// Summary: Encode and write a blob from a source file according to config.
    ///
    /// Inputs: Source file path, output writer, and timeout in seconds.
    ///
    /// Outputs: `Ok(plaintext_len)` when the source bytes are fully processed.
    ///
    /// Side effects: Reads from disk, may compress and or encrypt, and writes blob bytes to `out`.
    ///
    /// Error handling: Returns contextual errors for IO, compression, encryption, and timeouts.
    ///
    /// Ties to other methods: Called by the versioned store when persisting new blobs.
    ///
    /// Why this exists: Centralize blob encoding decisions so store logic stays simple.
    pub fn write_blob_from_file_to_writer(
        &self,
        src_path: &Path,
        out: &mut dyn Write,
        timeout_seconds: u64,
    ) -> Result<u64> {
        if let Some(cipher) = self.cipher.as_ref() {
            if self.write_compression_enabled {
                return cipher.encrypt_file_to_writer_zstd_chunked(
                    src_path,
                    out,
                    timeout_seconds,
                    self.write_compression_zstd_level,
                );
            }
            return cipher.encrypt_file_to_writer(src_path, out, timeout_seconds);
        }
        if self.write_compression_enabled {
            return compress_file_to_writer_zstd_chunked(
                src_path,
                out,
                timeout_seconds,
                self.write_compression_chunk_bytes,
                self.write_compression_zstd_level,
            );
        }
        copy_plain_file_to_writer(src_path, out, timeout_seconds)
    }

    /// Summary: Copy blob plaintext bytes to the provided writer, decoding as needed.
    ///
    /// Inputs: Blob path, output writer, and timeout in seconds.
    ///
    /// Outputs: Number of plaintext bytes written.
    ///
    /// Side effects: Reads blob bytes and may decrypt and or decompress.
    ///
    /// Error handling: Returns contextual errors for IO, decoding, and timeouts.
    ///
    /// Ties to other methods: Used by restore to reconstruct files from stored blobs.
    ///
    /// Why this exists: Restore must always operate on plaintext bytes regardless of blob encoding.
    pub fn copy_blob_plaintext_to_writer(
        &self,
        blob_path: &Path,
        out: &mut dyn Write,
        timeout_seconds: u64,
    ) -> Result<u64> {
        copy_blob_plaintext_to_writer_impl(
            blob_path,
            out,
            timeout_seconds,
            Instant::now(),
            self.cipher.as_ref(),
        )
    }

    /// Summary: Compute a SHA-256 of blob plaintext bytes, decoding as needed.
    ///
    /// Inputs: Blob path and timeout in seconds.
    ///
    /// Outputs: Hex SHA-256 of plaintext bytes.
    ///
    /// Side effects: Reads blob bytes and may decrypt and or decompress.
    ///
    /// Error handling: Returns contextual errors for IO, decoding, and timeouts.
    ///
    /// Ties to other methods: Used by scrub to compare stored blob contents to manifest hashes.
    ///
    /// Why this exists: Verification must hash the original plaintext even when blobs are stored encoded.
    pub fn sha256_plaintext_blob(&self, blob_path: &Path, timeout_seconds: u64) -> Result<String> {
        sha256_plaintext_blob_impl(
            blob_path,
            timeout_seconds,
            Instant::now(),
            self.cipher.as_ref(),
        )
    }
}

impl BlobCipher {
    /// Summary: Build a blob cipher from config when encryption is enabled.
    ///
    /// Inputs: a loaded config.
    ///
    /// Outputs: `Ok(Some(cipher))` when enabled, otherwise `Ok(None)`.
    ///
    /// Side effects: Reads key material from disk when enabled.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: versioned blob writes, restore reads, and scrub hashing.
    ///
    /// Why this exists: avoid repeatedly loading key files on hot paths.
    pub fn from_config(cfg: &Config) -> Result<Option<Self>> {
        if !cfg.encryption.enabled {
            return Ok(None);
        }
        let key_path = keyfile::resolve_key_path(cfg)?;
        let expected = cfg.encryption.key_id.as_deref().ok_or_else(|| {
            anyhow::anyhow!(
                "encryption::BlobCipher::from_config encryption enabled but encryption.key_id is missing"
            )
        })?;
        let info = keyfile::load_key_file(&key_path)?;
        if info.key_id != expected {
            anyhow::bail!(
                "encryption::BlobCipher::from_config key_id mismatch for {} (expected {}, got {})",
                redact_path(&key_path),
                expected,
                info.key_id
            );
        }
        Ok(Some(Self {
            key: info.key(),
            chunk_bytes: if cfg.compression.enabled {
                cfg.compression.blob_chunk_bytes.max(1)
            } else {
                cfg.encryption.blob_chunk_bytes.max(1)
            },
        }))
    }

    /// Summary: Encrypt a source file into the provided writer using a streaming AEAD format.
    ///
    /// Inputs: source file path, output writer, and timeout in seconds.
    ///
    /// Outputs: `Ok(plaintext_len)` when encryption completes.
    ///
    /// Side effects: Reads from source disk and writes ciphertext to the output writer.
    ///
    /// Error handling: Returns contextual errors on IO failures, encryption failures, or timeouts.
    ///
    /// Ties to other methods: versioned blob write pipeline.
    ///
    /// Why this exists: avoid loading entire files into memory while providing authenticated encryption.
    pub fn encrypt_file_to_writer(
        &self,
        src_path: &Path,
        out: &mut dyn Write,
        timeout_seconds: u64,
    ) -> Result<u64> {
        encrypt_file_to_writer_enc_stream(
            src_path,
            out,
            timeout_seconds,
            self.chunk_bytes,
            &self.key,
            ALG_XCHACHA20POLY1305_BE32,
            None,
        )
    }

    /// Summary: Encrypt a source file after per-chunk Zstd compression into the provided writer.
    ///
    /// Inputs: source file path, output writer, timeout seconds, and zstd compression level.
    ///
    /// Outputs: `Ok(plaintext_len)` when encoding completes.
    ///
    /// Side effects: Reads plaintext from disk, compresses, encrypts, and writes ciphertext to `out`.
    ///
    /// Error handling: Returns contextual errors for IO, compression, encryption, and timeouts.
    ///
    /// Ties to other methods: Used when both encryption and compression are enabled.
    ///
    /// Why this exists: Compression before encryption reduces destination size while keeping AEAD integrity meaningful.
    pub fn encrypt_file_to_writer_zstd_chunked(
        &self,
        src_path: &Path,
        out: &mut dyn Write,
        timeout_seconds: u64,
        zstd_level: i32,
    ) -> Result<u64> {
        encrypt_file_to_writer_enc_stream(
            src_path,
            out,
            timeout_seconds,
            self.chunk_bytes,
            &self.key,
            ALG_XCHACHA20POLY1305_BE32_ZSTD_CHUNKED,
            Some(zstd_level),
        )
    }

    /// Summary: copy_blob_plaintext_to_writer orchestrates this method's core behavior.
    ///
    /// Inputs: Method parameters and required receiver state.
    ///
    /// Outputs: Return value and observable result for callers.
    ///
    /// Side effects: None beyond this method's explicit operations.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: Invoked by and composes with adjacent module methods.
    ///
    /// Why this exists: Keeps this behavior isolated, testable, and reusable.
    pub fn copy_blob_plaintext_to_writer(
        &self,
        blob_path: &Path,
        out: &mut dyn Write,
        timeout_seconds: u64,
    ) -> Result<u64> {
        copy_blob_plaintext_to_writer_impl(
            blob_path,
            out,
            timeout_seconds,
            Instant::now(),
            Some(self),
        )
    }

    /// Summary: sha256_plaintext_blob orchestrates this method's core behavior.
    ///
    /// Inputs: Method parameters and required receiver state.
    ///
    /// Outputs: Return value and observable result for callers.
    ///
    /// Side effects: None beyond this method's explicit operations.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: Invoked by and composes with adjacent module methods.
    ///
    /// Why this exists: Keeps this behavior isolated, testable, and reusable.
    pub fn sha256_plaintext_blob(&self, blob_path: &Path, timeout_seconds: u64) -> Result<String> {
        sha256_plaintext_blob_impl(blob_path, timeout_seconds, Instant::now(), Some(self))
    }

    /// Summary: decrypt_stream_to_writer orchestrates this method's core behavior.
    ///
    /// Inputs: Method parameters and required receiver state.
    ///
    /// Outputs: Return value and observable result for callers.
    ///
    /// Side effects: None beyond this method's explicit operations.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: Invoked by and composes with adjacent module methods.
    ///
    /// Why this exists: Keeps this behavior isolated, testable, and reusable.
    fn decrypt_stream_to_writer(
        &self,
        file: &mut dyn Read,
        header: &EncHeader,
        out: &mut dyn Write,
        timeout_seconds: u64,
        start: Instant,
    ) -> Result<u64> {
        let aead = XChaCha20Poly1305::new(Key::from_slice(&self.key));
        let nonce = StreamNonce::<XChaCha20Poly1305, StreamBE32<XChaCha20Poly1305>>::from_slice(
            &header.nonce,
        );
        let mut dec = DecryptorBE32::from_aead(aead, nonce);
        let mut total: u64 = 0;
        loop {
            let (ct, is_last) =
                read_framed_chunk(file, timeout_seconds, start).with_context(|| {
                    "encryption::BlobCipher::decrypt_stream_to_writer failed reading framed chunk"
                })?;
            if is_last {
                let decoded = dec.decrypt_last(&ct[..]).context(
                    "encryption::BlobCipher::decrypt_stream_to_writer failed decrypting last chunk",
                )?;
                if header.alg == ALG_XCHACHA20POLY1305_BE32_ZSTD_CHUNKED {
                    let pt = decompress_zstd_payload(&decoded).context(
                        "encryption::BlobCipher::decrypt_stream_to_writer failed decompressing payload",
                    )?;
                    out.write_all(&pt).context(
                        "encryption::BlobCipher::decrypt_stream_to_writer failed writing decoded bytes",
                    )?;
                    total = total.saturating_add(pt.len() as u64);
                } else {
                    out.write_all(&decoded).context(
                        "encryption::BlobCipher::decrypt_stream_to_writer failed writing decoded bytes",
                    )?;
                    total = total.saturating_add(decoded.len() as u64);
                }
                break;
            }

            let decoded = dec.decrypt_next(&ct[..]).context(
                "encryption::BlobCipher::decrypt_stream_to_writer failed decrypting chunk",
            )?;
            if header.alg == ALG_XCHACHA20POLY1305_BE32_ZSTD_CHUNKED {
                let pt = decompress_zstd_payload(&decoded).context(
                    "encryption::BlobCipher::decrypt_stream_to_writer failed decompressing payload",
                )?;
                out.write_all(&pt).context(
                    "encryption::BlobCipher::decrypt_stream_to_writer failed writing decoded bytes",
                )?;
                total = total.saturating_add(pt.len() as u64);
            } else {
                out.write_all(&decoded).context(
                    "encryption::BlobCipher::decrypt_stream_to_writer failed writing decoded bytes",
                )?;
                total = total.saturating_add(decoded.len() as u64);
            }
        }
        Ok(total)
    }

    /// Summary: decrypt_stream_to_hasher orchestrates this method's core behavior.
    ///
    /// Inputs: Method parameters and required receiver state.
    ///
    /// Outputs: Return value and observable result for callers.
    ///
    /// Side effects: None beyond this method's explicit operations.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: Invoked by and composes with adjacent module methods.
    ///
    /// Why this exists: Keeps this behavior isolated, testable, and reusable.
    fn decrypt_stream_to_hasher(
        &self,
        file: &mut dyn Read,
        header: &EncHeader,
        hasher: &mut Sha256,
        timeout_seconds: u64,
        start: Instant,
    ) -> Result<()> {
        let aead = XChaCha20Poly1305::new(Key::from_slice(&self.key));
        let nonce = StreamNonce::<XChaCha20Poly1305, StreamBE32<XChaCha20Poly1305>>::from_slice(
            &header.nonce,
        );
        let mut dec = DecryptorBE32::from_aead(aead, nonce);
        loop {
            let (ct, is_last) =
                read_framed_chunk(file, timeout_seconds, start).with_context(|| {
                    "encryption::BlobCipher::decrypt_stream_to_hasher failed reading framed chunk"
                })?;
            if is_last {
                let decoded = dec.decrypt_last(&ct[..]).context(
                    "encryption::BlobCipher::decrypt_stream_to_hasher failed decrypting last chunk",
                )?;
                if header.alg == ALG_XCHACHA20POLY1305_BE32_ZSTD_CHUNKED {
                    let pt = decompress_zstd_payload(&decoded).context(
                        "encryption::BlobCipher::decrypt_stream_to_hasher failed decompressing payload",
                    )?;
                    hasher.update(&pt);
                } else {
                    hasher.update(&decoded);
                }
                break;
            }

            let decoded = dec.decrypt_next(&ct[..]).context(
                "encryption::BlobCipher::decrypt_stream_to_hasher failed decrypting chunk",
            )?;
            if header.alg == ALG_XCHACHA20POLY1305_BE32_ZSTD_CHUNKED {
                let pt = decompress_zstd_payload(&decoded).context(
                    "encryption::BlobCipher::decrypt_stream_to_hasher failed decompressing payload",
                )?;
                hasher.update(&pt);
            } else {
                hasher.update(&decoded);
            }
        }
        Ok(())
    }
}

/// Summary: blob_cipher_from_config orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
pub fn blob_cipher_from_config(cfg: &Config) -> Result<Option<BlobCipher>> {
    BlobCipher::from_config(cfg)
}

/// Summary: blob_codec_from_config orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
pub fn blob_codec_from_config(cfg: &Config) -> Result<BlobCodec> {
    BlobCodec::from_config(cfg)
}

/// Summary: copy_blob_plaintext_to_writer orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
pub fn copy_blob_plaintext_to_writer(
    cfg: &Config,
    blob_path: &Path,
    out: &mut dyn Write,
    timeout_seconds: u64,
) -> Result<u64> {
    BlobCodec::from_config(cfg)?.copy_blob_plaintext_to_writer(blob_path, out, timeout_seconds)
}

/// Summary: sha256_plaintext_blob orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
pub fn sha256_plaintext_blob(
    cfg: &Config,
    blob_path: &Path,
    timeout_seconds: u64,
) -> Result<String> {
    BlobCodec::from_config(cfg)?.sha256_plaintext_blob(blob_path, timeout_seconds)
}

#[derive(Clone, Copy)]
struct EncHeader {
    alg: u8,
    nonce: [u8; XCHACHA20POLY1305_STREAM_NONCE_LEN],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProbedMagic {
    Encrypted,
    Compressed,
    Plain,
}

/// Summary: encrypt_file_to_writer_enc_stream orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
fn encrypt_file_to_writer_enc_stream(
    src_path: &Path,
    out: &mut dyn Write,
    timeout_seconds: u64,
    chunk_bytes: usize,
    key: &[u8; 32],
    alg: u8,
    zstd_level: Option<i32>,
) -> Result<u64> {
    let start = Instant::now();
    let mut file = fs::File::open(src_path).with_context(|| {
        format!(
            "encryption::encrypt_file_to_writer_enc_stream failed to open source {}",
            redact_path(src_path)
        )
    })?;

    let aead = XChaCha20Poly1305::new(Key::from_slice(key));
    let mut nonce_bytes = [0u8; XCHACHA20POLY1305_STREAM_NONCE_LEN];
    use chacha20poly1305::aead::rand_core::RngCore as _;
    chacha20poly1305::aead::rand_core::OsRng.fill_bytes(&mut nonce_bytes);
    let nonce =
        StreamNonce::<XChaCha20Poly1305, StreamBE32<XChaCha20Poly1305>>::from_slice(&nonce_bytes);
    let mut enc = EncryptorBE32::from_aead(aead, nonce);

    out.write_all(ENC_MAGIC)
        .context("encryption::encrypt_file_to_writer_enc_stream failed to write magic")?;
    out.write_all(&[ENC_VERSION, alg])
        .context("encryption::encrypt_file_to_writer_enc_stream failed to write header bytes")?;
    out.write_all(&nonce_bytes)
        .context("encryption::encrypt_file_to_writer_enc_stream failed to write nonce")?;

    let mut buf_a = vec![0u8; chunk_bytes.max(1)];
    let mut buf_b = vec![0u8; chunk_bytes.max(1)];
    let mut a_len = read_chunk(&mut file, &mut buf_a, timeout_seconds, start)?;
    let mut total_plain: u64 = 0;

    if a_len == 0 {
        let ct = match zstd_level {
            None => enc.encrypt_last(&[] as &[u8]),
            Some(level) => {
                let payload = build_zstd_payload(&[], level)?;
                enc.encrypt_last(payload.as_slice())
            }
        }
        .context(
            "encryption::encrypt_file_to_writer_enc_stream failed encrypting empty last chunk",
        )?;
        write_framed_chunk(out, &ct, true)?;
        return Ok(0);
    }

    loop {
        let b_len = read_chunk(&mut file, &mut buf_b, timeout_seconds, start)?;
        let is_last = b_len == 0;
        let pt = &buf_a[..a_len];
        total_plain = total_plain.saturating_add(a_len as u64);

        match zstd_level {
            None => {
                if is_last {
                    let ct = enc.encrypt_last(pt).context(
                        "encryption::encrypt_file_to_writer_enc_stream failed encrypting last chunk",
                    )?;
                    write_framed_chunk(out, &ct, true)?;
                    break;
                }
                let ct = enc.encrypt_next(pt).context(
                    "encryption::encrypt_file_to_writer_enc_stream failed encrypting next chunk",
                )?;
                write_framed_chunk(out, &ct, false)?;
            }
            Some(level) => {
                let payload = build_zstd_payload(pt, level)?;
                if is_last {
                    let ct = enc.encrypt_last(payload.as_slice()).context(
                        "encryption::encrypt_file_to_writer_enc_stream failed encrypting last chunk",
                    )?;
                    write_framed_chunk(out, &ct, true)?;
                    break;
                }
                let ct = enc.encrypt_next(payload.as_slice()).context(
                    "encryption::encrypt_file_to_writer_enc_stream failed encrypting next chunk",
                )?;
                write_framed_chunk(out, &ct, false)?;
            }
        }

        std::mem::swap(&mut buf_a, &mut buf_b);
        a_len = b_len;
    }

    Ok(total_plain)
}

/// Summary: copy_plain_file_to_writer orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
fn copy_plain_file_to_writer(
    src_path: &Path,
    out: &mut dyn Write,
    timeout_seconds: u64,
) -> Result<u64> {
    let start = Instant::now();
    let mut file = fs::File::open(src_path).with_context(|| {
        format!(
            "encryption::copy_plain_file_to_writer failed to open source {}",
            redact_path(src_path)
        )
    })?;
    copy_plain_stream(&mut file, out, timeout_seconds, start)
}

/// Summary: compress_file_to_writer_zstd_chunked orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
fn compress_file_to_writer_zstd_chunked(
    src_path: &Path,
    out: &mut dyn Write,
    timeout_seconds: u64,
    chunk_bytes: usize,
    zstd_level: i32,
) -> Result<u64> {
    let start = Instant::now();
    let mut file = fs::File::open(src_path).with_context(|| {
        format!(
            "encryption::compress_file_to_writer_zstd_chunked failed to open source {}",
            redact_path(src_path)
        )
    })?;
    out.write_all(COMP_MAGIC)
        .context("encryption::compress_file_to_writer_zstd_chunked failed to write magic")?;
    out.write_all(&[COMP_VERSION, COMP_ALG_ZSTD_CHUNKED])
        .context("encryption::compress_file_to_writer_zstd_chunked failed to write header bytes")?;

    let mut buf_a = vec![0u8; chunk_bytes.max(1)];
    let mut buf_b = vec![0u8; chunk_bytes.max(1)];
    let mut a_len = read_chunk(&mut file, &mut buf_a, timeout_seconds, start)?;
    let mut total_plain: u64 = 0;
    if a_len == 0 {
        let payload = build_zstd_payload(&[], zstd_level)?;
        write_framed_chunk(out, payload.as_slice(), true)?;
        return Ok(0);
    }
    loop {
        let b_len = read_chunk(&mut file, &mut buf_b, timeout_seconds, start)?;
        let is_last = b_len == 0;
        let pt = &buf_a[..a_len];
        total_plain = total_plain.saturating_add(a_len as u64);

        let payload = build_zstd_payload(pt, zstd_level)?;
        write_framed_chunk(out, payload.as_slice(), is_last)?;
        if is_last {
            break;
        }
        std::mem::swap(&mut buf_a, &mut buf_b);
        a_len = b_len;
    }
    Ok(total_plain)
}

/// Summary: copy_blob_plaintext_to_writer_impl orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
fn copy_blob_plaintext_to_writer_impl(
    blob_path: &Path,
    out: &mut dyn Write,
    timeout_seconds: u64,
    start: Instant,
    cipher: Option<&BlobCipher>,
) -> Result<u64> {
    let mut file = fs::File::open(blob_path).with_context(|| {
        format!(
            "encryption::copy_blob_plaintext_to_writer failed to open blob {}",
            redact_path(blob_path)
        )
    })?;
    let (kind, prefix) = probe_magic_prefix(&mut file, timeout_seconds, start)?;
    match kind {
        ProbedMagic::Encrypted => {
            let Some(cipher) = cipher else {
                anyhow::bail!(
                    "encryption::copy_blob_plaintext_to_writer encountered encrypted blob {} but encryption is disabled or key is unavailable",
                    redact_path(blob_path)
                );
            };
            let header = read_encrypted_header(&mut file, timeout_seconds, start)?;
            cipher.decrypt_stream_to_writer(&mut file, &header, out, timeout_seconds, start)
        }
        ProbedMagic::Compressed => {
            let _header = read_compressed_header(&mut file, timeout_seconds, start)?;
            decompress_stream_to_writer(&mut file, out, timeout_seconds, start).with_context(|| {
                format!(
                    "encryption::copy_blob_plaintext_to_writer failed decompressing blob {}",
                    redact_path(blob_path)
                )
            })
        }
        ProbedMagic::Plain => {
            if !prefix.is_empty() {
                out.write_all(&prefix).context(
                    "encryption::copy_blob_plaintext_to_writer failed writing prefix bytes",
                )?;
            }
            copy_plain_stream(&mut file, out, timeout_seconds, start)
                .with_context(|| {
                    format!(
                        "encryption::copy_blob_plaintext_to_writer failed copying plaintext blob {}",
                        redact_path(blob_path)
                    )
                })
                .map(|n| n.saturating_add(prefix.len() as u64))
        }
    }
}

/// Summary: sha256_plaintext_blob_impl orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
fn sha256_plaintext_blob_impl(
    blob_path: &Path,
    timeout_seconds: u64,
    start: Instant,
    cipher: Option<&BlobCipher>,
) -> Result<String> {
    let mut file = fs::File::open(blob_path).with_context(|| {
        format!(
            "encryption::sha256_plaintext_blob failed to open blob {}",
            redact_path(blob_path)
        )
    })?;
    let mut hasher = Sha256::new();
    let (kind, prefix) = probe_magic_prefix(&mut file, timeout_seconds, start)?;
    match kind {
        ProbedMagic::Encrypted => {
            let Some(cipher) = cipher else {
                anyhow::bail!(
                    "encryption::sha256_plaintext_blob encountered encrypted blob {} but encryption is disabled or key is unavailable",
                    redact_path(blob_path)
                );
            };
            let header = read_encrypted_header(&mut file, timeout_seconds, start)?;
            cipher
                .decrypt_stream_to_hasher(&mut file, &header, &mut hasher, timeout_seconds, start)
                .with_context(|| {
                    format!(
                        "encryption::sha256_plaintext_blob failed decrypting blob {}",
                        redact_path(blob_path)
                    )
                })?;
        }
        ProbedMagic::Compressed => {
            let _header = read_compressed_header(&mut file, timeout_seconds, start)?;
            decompress_stream_to_hasher(&mut file, &mut hasher, timeout_seconds, start)
                .with_context(|| {
                    format!(
                        "encryption::sha256_plaintext_blob failed decompressing blob {}",
                        redact_path(blob_path)
                    )
                })?;
        }
        ProbedMagic::Plain => {
            if !prefix.is_empty() {
                hasher.update(&prefix);
            }
            let mut buf = vec![0u8; 64 * 1024];
            loop {
                if timed_out(timeout_seconds, start) {
                    anyhow::bail!(
                        "encryption::sha256_plaintext_blob timed out after {}s hashing {}",
                        timeout_seconds,
                        redact_path(blob_path)
                    );
                }
                let n = file.read(&mut buf).with_context(|| {
                    format!(
                        "encryption::sha256_plaintext_blob failed reading {}",
                        redact_path(blob_path)
                    )
                })?;
                if n == 0 {
                    break;
                }
                hasher.update(&buf[..n]);
            }
        }
    }
    let digest = hasher.finalize();
    Ok(crate::hashing::hex_lower(digest.as_slice()))
}

/// Summary: read_encrypted_header orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
fn read_encrypted_header(
    input: &mut dyn Read,
    timeout_seconds: u64,
    start: Instant,
) -> Result<EncHeader> {
    let mut meta = [0u8; 2];
    read_exact_timeout(input, &mut meta, timeout_seconds, start)
        .context("encryption::read_encrypted_header failed to read header bytes")?;
    let ver = meta[0];
    let alg = meta[1];
    if ver != ENC_VERSION {
        anyhow::bail!(
            "encryption::read_encrypted_header unsupported blob version {}",
            ver
        );
    }
    if alg != ALG_XCHACHA20POLY1305_BE32 && alg != ALG_XCHACHA20POLY1305_BE32_ZSTD_CHUNKED {
        anyhow::bail!(
            "encryption::read_encrypted_header unsupported blob algorithm {}",
            alg
        );
    }
    let mut nonce_bytes = [0u8; XCHACHA20POLY1305_STREAM_NONCE_LEN];
    read_exact_timeout(input, &mut nonce_bytes, timeout_seconds, start)
        .context("encryption::read_encrypted_header failed to read nonce")?;
    Ok(EncHeader {
        alg,
        nonce: nonce_bytes,
    })
}

/// Summary: read_compressed_header orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
fn read_compressed_header(
    input: &mut dyn Read,
    timeout_seconds: u64,
    start: Instant,
) -> Result<u8> {
    let mut meta = [0u8; 2];
    read_exact_timeout(input, &mut meta, timeout_seconds, start)
        .context("encryption::read_compressed_header failed to read header bytes")?;
    let ver = meta[0];
    let alg = meta[1];
    if ver != COMP_VERSION {
        anyhow::bail!(
            "encryption::read_compressed_header unsupported blob version {}",
            ver
        );
    }
    if alg != COMP_ALG_ZSTD_CHUNKED {
        anyhow::bail!(
            "encryption::read_compressed_header unsupported blob algorithm {}",
            alg
        );
    }
    Ok(alg)
}

/// Summary: probe_magic_prefix orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
fn probe_magic_prefix(
    input: &mut dyn Read,
    timeout_seconds: u64,
    start: Instant,
) -> Result<(ProbedMagic, Vec<u8>)> {
    let mut magic = [0u8; 8];
    let mut filled = 0usize;
    while filled < magic.len() {
        if timed_out(timeout_seconds, start) {
            anyhow::bail!(
                "encryption::probe_magic_prefix timed out after {}s while probing blob header",
                timeout_seconds
            );
        }
        let n = input
            .read(&mut magic[filled..])
            .context("encryption::probe_magic_prefix read failed")?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    if filled == 0 {
        return Ok((ProbedMagic::Plain, Vec::new()));
    }
    if filled < magic.len() {
        return Ok((ProbedMagic::Plain, magic[..filled].to_vec()));
    }
    if &magic == ENC_MAGIC {
        return Ok((ProbedMagic::Encrypted, Vec::new()));
    }
    if &magic == COMP_MAGIC {
        return Ok((ProbedMagic::Compressed, Vec::new()));
    }
    Ok((ProbedMagic::Plain, magic.to_vec()))
}

/// Summary: build_zstd_payload orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
fn build_zstd_payload(pt: &[u8], level: i32) -> Result<Vec<u8>> {
    if pt.len() > u32::MAX as usize {
        anyhow::bail!(
            "encryption::build_zstd_payload plaintext chunk too large ({} bytes)",
            pt.len()
        );
    }
    let mut out = Vec::with_capacity(4);
    out.extend_from_slice(&(pt.len() as u32).to_be_bytes());
    if !pt.is_empty() {
        let ct = zstd::bulk::compress(pt, level)
            .context("encryption::build_zstd_payload zstd compress failed")?;
        out.extend_from_slice(&ct);
    }
    Ok(out)
}

/// Summary: decompress_zstd_payload orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
fn decompress_zstd_payload(payload: &[u8]) -> Result<Vec<u8>> {
    if payload.len() < 4 {
        anyhow::bail!(
            "encryption::decompress_zstd_payload invalid payload length {} (expected >= 4)",
            payload.len()
        );
    }
    let pt_len = u32::from_be_bytes(payload[0..4].try_into().unwrap()) as usize;
    if pt_len > MAX_DECOMPRESSED_CHUNK_BYTES {
        anyhow::bail!(
            "encryption::decompress_zstd_payload plaintext length {} exceeds max {}",
            pt_len,
            MAX_DECOMPRESSED_CHUNK_BYTES
        );
    }
    let ct = &payload[4..];
    if pt_len == 0 {
        if ct.is_empty() {
            return Ok(Vec::new());
        }
        return zstd::bulk::decompress(ct, 0)
            .context("encryption::decompress_zstd_payload zstd decompress failed");
    }
    zstd::bulk::decompress(ct, pt_len)
        .context("encryption::decompress_zstd_payload zstd decompress failed")
}

/// Summary: decompress_stream_to_writer orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
fn decompress_stream_to_writer(
    input: &mut dyn Read,
    out: &mut dyn Write,
    timeout_seconds: u64,
    start: Instant,
) -> Result<u64> {
    let mut total: u64 = 0;
    loop {
        let (payload, is_last) = read_framed_chunk(input, timeout_seconds, start)
            .context("encryption::decompress_stream_to_writer failed reading framed chunk")?;
        let pt = decompress_zstd_payload(payload.as_slice())
            .context("encryption::decompress_stream_to_writer failed decompressing chunk")?;
        out.write_all(&pt)
            .context("encryption::decompress_stream_to_writer failed writing output")?;
        total = total.saturating_add(pt.len() as u64);
        if is_last {
            break;
        }
    }
    Ok(total)
}

/// Summary: decompress_stream_to_hasher orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
fn decompress_stream_to_hasher(
    input: &mut dyn Read,
    hasher: &mut Sha256,
    timeout_seconds: u64,
    start: Instant,
) -> Result<()> {
    loop {
        let (payload, is_last) = read_framed_chunk(input, timeout_seconds, start)
            .context("encryption::decompress_stream_to_hasher failed reading framed chunk")?;
        let pt = decompress_zstd_payload(payload.as_slice())
            .context("encryption::decompress_stream_to_hasher failed decompressing chunk")?;
        hasher.update(&pt);
        if is_last {
            break;
        }
    }
    Ok(())
}

/// Summary: copy_plain_stream orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
fn copy_plain_stream(
    input: &mut dyn Read,
    out: &mut dyn Write,
    timeout_seconds: u64,
    start: Instant,
) -> Result<u64> {
    let mut buf = vec![0u8; 64 * 1024];
    let mut total: u64 = 0;
    loop {
        if timed_out(timeout_seconds, start) {
            anyhow::bail!(
                "encryption::copy_plain_stream timed out after {}s copying blob bytes",
                timeout_seconds
            );
        }
        let n = input
            .read(&mut buf)
            .context("encryption::copy_plain_stream failed reading input")?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])
            .context("encryption::copy_plain_stream failed writing output")?;
        total = total.saturating_add(n as u64);
    }
    Ok(total)
}

/// Summary: read_chunk orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
fn read_chunk(
    input: &mut fs::File,
    buf: &mut [u8],
    timeout_seconds: u64,
    start: Instant,
) -> Result<usize> {
    if timed_out(timeout_seconds, start) {
        anyhow::bail!(
            "encryption::read_chunk timed out after {}s reading source bytes",
            timeout_seconds
        );
    }
    input
        .read(buf)
        .context("encryption::read_chunk failed reading source bytes")
}

/// Summary: read_exact_timeout orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
fn read_exact_timeout(
    input: &mut dyn Read,
    buf: &mut [u8],
    timeout_seconds: u64,
    start: Instant,
) -> Result<()> {
    let mut filled = 0usize;
    while filled < buf.len() {
        if timed_out(timeout_seconds, start) {
            anyhow::bail!(
                "encryption::read_exact_timeout timed out after {}s while reading encoded blob",
                timeout_seconds
            );
        }
        let n = input
            .read(&mut buf[filled..])
            .context("encryption::read_exact_timeout read failed")?;
        if n == 0 {
            anyhow::bail!("encryption::read_exact_timeout unexpected EOF");
        }
        filled += n;
    }
    Ok(())
}

/// Summary: write_framed_chunk orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
fn write_framed_chunk(out: &mut dyn Write, chunk: &[u8], is_last: bool) -> Result<()> {
    if chunk.len() > 0x7FFF_FFFF {
        anyhow::bail!(
            "encryption::write_framed_chunk chunk too large ({} bytes)",
            chunk.len()
        );
    }
    let mut len = chunk.len() as u32;
    if is_last {
        len |= 0x8000_0000;
    }
    out.write_all(&len.to_be_bytes())
        .context("encryption::write_framed_chunk failed to write length")?;
    out.write_all(chunk)
        .context("encryption::write_framed_chunk failed to write chunk bytes")?;
    Ok(())
}

/// Summary: read_framed_chunk orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
fn read_framed_chunk(
    input: &mut dyn Read,
    timeout_seconds: u64,
    start: Instant,
) -> Result<(Vec<u8>, bool)> {
    let mut len_bytes = [0u8; 4];
    read_exact_timeout(input, &mut len_bytes, timeout_seconds, start)
        .context("encryption::read_framed_chunk failed to read length")?;
    let raw = u32::from_be_bytes(len_bytes);
    let is_last = (raw & 0x8000_0000) != 0;
    let len = (raw & 0x7FFF_FFFF) as usize;
    let mut buf = vec![0u8; len];
    read_exact_timeout(input, &mut buf, timeout_seconds, start)
        .context("encryption::read_framed_chunk failed to read chunk bytes")?;
    Ok((buf, is_last))
}

/// Summary: timed_out orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
fn timed_out(timeout_seconds: u64, start: Instant) -> bool {
    timeout_seconds > 0 && start.elapsed() > Duration::from_secs(timeout_seconds)
}
