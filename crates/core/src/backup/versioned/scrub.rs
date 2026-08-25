use super::model::{Manifest, ManifestEntryKind, VersionIndex};
use super::operation_lock::acquire_store_leases;
use super::store::{blob_path, blobs_root, sources_root, store_root};
use crate::config::model::{Config, Destination, HashingTuning};
use crate::encryption::blobs::BlobCodec;
use crate::hashing;
use crate::io::BlockingIoPolicy;
use anyhow::{Context, Result};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ScrubMode {
    #[default]
    Sampled,
    Full,
}

#[derive(Debug, Clone, Default)]
pub struct ScrubResult {
    pub mode: ScrubMode,
    pub manifests_checked: usize,
    pub manifests_bad: usize,
    pub referenced_blobs: usize,
    pub missing_blobs: usize,
    pub blobs_hashed: usize,
    pub hash_mismatches: usize,
}

/// Walk committed manifests and verify that referenced blobs still exist and decode correctly.
pub fn scrub_versioned_store(
    cfg: &Config,
    hashing: &HashingTuning,
    mode: ScrubMode,
    sample_blobs: usize,
    sample_versions_per_source: usize,
    seed: u64,
) -> Result<ScrubResult> {
    let destinations_by_id: HashMap<&str, &Destination> = cfg
        .destinations
        .iter()
        .map(|d| (d.id.as_str(), d))
        .collect();

    let mut enabled_destinations = Vec::new();
    for watched in cfg.watched.iter().filter(|watched| watched.enabled) {
        let destination = destinations_by_id
            .get(watched.destination_id.as_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "versioned::scrub_versioned_store missing destination id {} for {:?}",
                    watched.destination_id,
                    watched.path
                )
            })?;
        enabled_destinations.push(destination.path.as_path());
    }
    let lock_policy = BlockingIoPolicy::from_config(cfg);
    let _operation_leases = acquire_store_leases(enabled_destinations, &lock_policy)
        .context("versioned::scrub_versioned_store could not serialize destination stores")?;

    let blob_codec = BlobCodec::from_config(cfg)
        .context("versioned::scrub_versioned_store failed to initialize blob codec")?;

    let mut referenced_by_dest: HashMap<PathBuf, BTreeSet<String>> = HashMap::new();
    let mut manifests_checked = 0usize;
    let mut manifests_bad = 0usize;

    for watched in cfg.watched.iter().filter(|w| w.enabled) {
        let dest = destinations_by_id
            .get(watched.destination_id.as_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "versioned::scrub_versioned_store missing destination id {} for {:?}",
                    watched.destination_id,
                    watched.path
                )
            })?;
        let store = store_root(&dest.path);
        let src_id = hashing::sha256_hex(watched.path.to_string_lossy().as_bytes());
        let source_root = sources_root(&store).join(&src_id);
        let index_path = source_root.join("index.json");
        if !index_path.exists() {
            continue;
        }
        let raw = fs::read_to_string(&index_path).with_context(|| {
            format!(
                "versioned::scrub_versioned_store failed to read {:?}",
                index_path
            )
        })?;
        let index: VersionIndex = serde_json::from_str(&raw).with_context(|| {
            format!(
                "versioned::scrub_versioned_store failed to parse {:?}",
                index_path
            )
        })?;
        let manifests_root = source_root.join("manifests");
        let versions: &[super::model::VersionInfo] = if mode == ScrubMode::Full {
            &index.versions
        } else {
            let n = sample_versions_per_source.max(1);
            let start = index.versions.len().saturating_sub(n);
            &index.versions[start..]
        };
        for v in versions.iter() {
            let path = manifests_root.join(format!("{}.json", v.id));
            if !path.exists() {
                manifests_bad += 1;
                continue;
            }
            let raw = match fs::read_to_string(&path) {
                Ok(r) => r,
                Err(_) => {
                    manifests_bad += 1;
                    continue;
                }
            };
            let manifest: Manifest = match serde_json::from_str(&raw) {
                Ok(m) => m,
                Err(_) => {
                    manifests_bad += 1;
                    continue;
                }
            };
            manifests_checked += 1;
            let set = referenced_by_dest.entry(dest.path.clone()).or_default();
            for e in manifest.entries.values() {
                if e.kind != ManifestEntryKind::File {
                    continue;
                }
                if let Some(h) = e.sha256.as_ref() {
                    set.insert(h.clone());
                } else {
                    manifests_bad += 1;
                }
            }
        }
    }

    let mut missing_blobs = 0usize;
    let mut blobs_hashed = 0usize;
    let mut hash_mismatches = 0usize;

    for (dest_root, hashes) in referenced_by_dest.iter() {
        let store = store_root(dest_root);
        let blobs = blobs_root(&store);
        let referenced: Vec<String> = hashes.iter().cloned().collect();
        let selected = select_hashes_for_mode(&referenced, sample_blobs, mode, seed)?;
        let selected_set: HashSet<&str> = selected.iter().map(|s| s.as_str()).collect();

        for h in referenced.iter() {
            let blob = blob_path(&blobs, h);
            if !blob.exists() {
                missing_blobs += 1;
                continue;
            }
            if mode == ScrubMode::Full || selected_set.contains(h.as_str()) {
                blobs_hashed += 1;
                let computed = blob_codec.sha256_plaintext_blob(&blob, hashing.timeout_seconds)?;
                if computed != *h {
                    hash_mismatches += 1;
                }
            }
        }
    }

    Ok(ScrubResult {
        mode,
        manifests_checked,
        manifests_bad,
        referenced_blobs: referenced_by_dest.values().map(|s| s.len()).sum(),
        missing_blobs,
        blobs_hashed,
        hash_mismatches,
    })
}

/// Select which referenced blob hashes to verify in sampled scrub mode.
fn select_hashes_for_mode(
    hashes: &[String],
    sample_blobs: usize,
    mode: ScrubMode,
    seed: u64,
) -> Result<Vec<String>> {
    if mode == ScrubMode::Full || hashes.is_empty() {
        return Ok(Vec::new());
    }
    let max = sample_blobs.min(hashes.len());
    if max == hashes.len() {
        return Ok(hashes.to_vec());
    }
    let mut out: Vec<String> = Vec::with_capacity(max);
    let start = (seed % hashes.len() as u64) as usize;
    let mut step = 97usize % hashes.len();
    if step == 0 {
        step = 1;
    }
    while greatest_common_divisor(step, hashes.len()) != 1 {
        step = (step + 1) % hashes.len();
        if step == 0 {
            step = 1;
        }
    }
    let mut idx = start;
    while out.len() < max {
        out.push(hashes[idx].clone());
        idx = (idx + step) % hashes.len();
    }
    Ok(out)
}

fn greatest_common_divisor(mut left: usize, mut right: usize) -> usize {
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left
}

#[cfg(test)]
mod tests {
    use super::{select_hashes_for_mode, ScrubMode};
    use std::collections::HashSet;

    fn hashes(len: usize) -> Vec<String> {
        (0..len).map(|index| format!("hash-{index}")).collect()
    }

    #[test]
    fn sampled_selection_terminates_with_unique_requested_cardinality() {
        let hashes = hashes(194);
        let selected = select_hashes_for_mode(&hashes, 193, ScrubMode::Sampled, 42).unwrap();

        assert_eq!(selected.len(), 193);
        assert_eq!(selected.iter().collect::<HashSet<_>>().len(), 193);
        assert_eq!(
            selected,
            select_hashes_for_mode(&hashes, 193, ScrubMode::Sampled, 42).unwrap()
        );
    }

    #[test]
    fn sampled_selection_caps_at_input_length() {
        let hashes = hashes(194);
        let selected = select_hashes_for_mode(&hashes, 200, ScrubMode::Sampled, 42).unwrap();

        assert_eq!(selected, hashes);
    }

    #[test]
    fn zero_sample_and_full_mode_select_no_hashes() {
        let hashes = hashes(194);

        assert!(select_hashes_for_mode(&hashes, 0, ScrubMode::Sampled, 42)
            .unwrap()
            .is_empty());
        assert!(select_hashes_for_mode(&hashes, 193, ScrubMode::Full, 42)
            .unwrap()
            .is_empty());
    }
}

// Plainfile hashing is handled by `BlobCodec`, which also supports decoding compressed and or encrypted blobs.
