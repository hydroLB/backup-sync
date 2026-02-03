use super::model::{Manifest, ManifestEntryKind, VersionInfo};
use super::store::{blob_path, blobs_root, sha256_hex, sources_root, store_root};
use crate::config::model::{Config, Destination, WatchedKind};
use anyhow::{Context, Result};
use filetime::{set_file_mtime, FileTime};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreMode {
    InPlace,
    ToDirectory,
}

#[derive(Debug, Clone)]
pub struct RestoreRequest {
    pub source_path: PathBuf,
    pub version_id: String,
    pub mode: RestoreMode,
    pub target_dir: Option<PathBuf>,
}

#[derive(Debug, Default, Clone)]
pub struct RestoreResult {
    pub files_written: usize,
    pub files_removed: usize,
    pub dirs_created: usize,
}

pub fn list_versions(cfg: &Config) -> Result<Vec<(PathBuf, Vec<VersionInfo>)>> {
    let destinations_by_id: HashMap<&str, &Destination> = cfg
        .destinations
        .iter()
        .map(|d| (d.id.as_str(), d))
        .collect();

    let mut out: Vec<(PathBuf, Vec<VersionInfo>)> = Vec::new();
    for watched in cfg.watched.iter().filter(|w| w.enabled) {
        if !matches!(watched.kind, WatchedKind::Directory) {
            continue;
        }
        let dest = destinations_by_id
            .get(watched.destination_id.as_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "versioned::list_versions missing destination id {} for watched path {}",
                    watched.destination_id,
                    watched.path.display()
                )
            })?;
        let store_root = store_root(&dest.path);
        let source_id = sha256_hex(watched.path.to_string_lossy().as_bytes());
        let index_path = sources_root(&store_root)
            .join(&source_id)
            .join("index.json");
        if !index_path.exists() {
            out.push((watched.path.clone(), Vec::new()));
            continue;
        }
        let raw = fs::read_to_string(&index_path)
            .with_context(|| format!("versioned::list_versions failed to read {:?}", index_path))?;
        let index: super::model::VersionIndex = serde_json::from_str(&raw).with_context(|| {
            format!("versioned::list_versions failed to parse {:?}", index_path)
        })?;
        out.push((watched.path.clone(), index.versions));
    }
    Ok(out)
}

pub fn restore_version(cfg: &Config, req: &RestoreRequest) -> Result<RestoreResult> {
    let watched = cfg
        .watched
        .iter()
        .find(|w| w.enabled && w.path == req.source_path)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "versioned::restore_version source path not configured: {:?}",
                req.source_path
            )
        })?;
    if !matches!(watched.kind, WatchedKind::Directory) {
        anyhow::bail!(
            "versioned::restore_version only directory watched paths are supported: {:?}",
            watched.path
        );
    }

    let destinations_by_id: HashMap<&str, &Destination> = cfg
        .destinations
        .iter()
        .map(|d| (d.id.as_str(), d))
        .collect();
    let dest = destinations_by_id
        .get(watched.destination_id.as_str())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "versioned::restore_version missing destination id {} for {:?}",
                watched.destination_id,
                watched.path
            )
        })?;

    let store_root = store_root(&dest.path);
    let blobs_root = blobs_root(&store_root);
    let source_id = sha256_hex(req.source_path.to_string_lossy().as_bytes());
    let manifests_root = sources_root(&store_root).join(&source_id).join("manifests");
    let manifest_path = manifests_root.join(format!("{}.json", req.version_id));
    let raw = fs::read_to_string(&manifest_path).with_context(|| {
        format!(
            "versioned::restore_version failed to read manifest {:?}",
            manifest_path
        )
    })?;
    let manifest: Manifest = serde_json::from_str(&raw).with_context(|| {
        format!(
            "versioned::restore_version failed to parse manifest {:?}",
            manifest_path
        )
    })?;

    let restore_root = match req.mode {
        RestoreMode::InPlace => req.source_path.clone(),
        RestoreMode::ToDirectory => req
            .target_dir
            .clone()
            .context("versioned::restore_version missing target_dir for ToDirectory")?,
    };
    fs::create_dir_all(&restore_root).with_context(|| {
        format!(
            "versioned::restore_version failed to create restore root {:?}",
            restore_root
        )
    })?;

    let mut result = RestoreResult::default();
    if req.mode == RestoreMode::InPlace {
        result.files_removed += remove_extraneous(&restore_root, &manifest)?;
    }

    for entry in manifest.entries.values() {
        if entry.kind != ManifestEntryKind::Dir {
            continue;
        }
        let dir_path = restore_root.join(&entry.rel_path);
        if !dir_path.exists() {
            fs::create_dir_all(&dir_path).with_context(|| {
                format!(
                    "versioned::restore_version failed to create directory {:?}",
                    dir_path
                )
            })?;
            result.dirs_created += 1;
        }
    }

    let mut missing: Vec<String> = Vec::new();
    for entry in manifest.entries.values() {
        if entry.kind != ManifestEntryKind::File {
            continue;
        }
        let hash = match entry.sha256.as_deref() {
            None => {
                missing.push(format!("{}: missing sha256 in manifest", entry.rel_path));
                continue;
            }
            Some(h) => h,
        };
        let blob = blob_path(&blobs_root, hash);
        if !blob.exists() {
            missing.push(format!("{}: missing blob {}", entry.rel_path, hash));
            continue;
        }
        let out_path = restore_root.join(&entry.rel_path);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "versioned::restore_version failed to create parent dir {:?}",
                    parent
                )
            })?;
        }
        write_file_atomic_from_blob(&blob, &out_path)?;
        restore_mtime(&out_path, entry.mtime_unix, entry.mtime_nanos)?;
        result.files_written += 1;
    }

    if !missing.is_empty() {
        anyhow::bail!(
            "versioned::restore_version cannot restore due to missing content:\n{}",
            missing.join("\n")
        );
    }

    Ok(result)
}

fn write_file_atomic_from_blob(blob_path: &Path, out_path: &Path) -> Result<()> {
    let parent = out_path
        .parent()
        .context("versioned::write_file_atomic_from_blob missing parent")?;
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "versioned::write_file_atomic_from_blob failed to create parent {:?}",
            parent
        )
    })?;

    let mut blob = fs::File::open(blob_path).with_context(|| {
        format!(
            "versioned::write_file_atomic_from_blob failed to open blob {:?}",
            blob_path
        )
    })?;

    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .context("versioned::write_file_atomic_from_blob failed to create temp file")?;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = blob.read(&mut buf).with_context(|| {
            format!(
                "versioned::write_file_atomic_from_blob failed reading blob {:?}",
                blob_path
            )
        })?;
        if n == 0 {
            break;
        }
        temp.write_all(&buf[..n]).with_context(|| {
            format!(
                "versioned::write_file_atomic_from_blob failed writing temp file for {:?}",
                out_path
            )
        })?;
    }
    temp.flush().ok();

    let _ = fs::remove_file(out_path);
    temp.persist(out_path).map_err(|e| {
        anyhow::anyhow!(
            "versioned::write_file_atomic_from_blob failed to persist {:?}: {}",
            out_path,
            e
        )
    })?;
    Ok(())
}

fn remove_extraneous(root: &Path, manifest: &Manifest) -> Result<usize> {
    let expected_files: HashSet<String> = manifest
        .entries
        .values()
        .filter(|e| e.kind == ManifestEntryKind::File)
        .map(|e| e.rel_path.clone())
        .collect();

    let mut expected_dirs: HashSet<String> = HashSet::new();
    for e in manifest.entries.values() {
        if e.kind == ManifestEntryKind::Dir {
            expected_dirs.insert(e.rel_path.clone());
        } else {
            let mut current = Path::new(&e.rel_path);
            while let Some(parent) = current.parent() {
                if parent.as_os_str().is_empty() {
                    break;
                }
                expected_dirs.insert(parent.to_string_lossy().replace('\\', "/"));
                current = parent;
            }
        }
    }

    let mut removed = 0usize;
    let mut paths: Vec<PathBuf> = Vec::new();
    for item in walkdir::WalkDir::new(root).min_depth(1).follow_links(false) {
        let item = item?;
        paths.push(item.path().to_path_buf());
    }
    paths.sort_by_key(|p| std::cmp::Reverse(p.components().count()));

    for path in paths {
        let rel = path.strip_prefix(root).unwrap_or(&path);
        let rel_s = rel.to_string_lossy().replace('\\', "/");
        if path.is_file() {
            if !expected_files.contains(rel_s.as_str()) {
                fs::remove_file(&path).ok();
                removed += 1;
            }
        } else if path.is_dir() && !expected_dirs.contains(rel_s.as_str()) {
            fs::remove_dir_all(&path).ok();
        }
    }
    Ok(removed)
}

fn restore_mtime(path: &Path, mtime_unix: i64, mtime_nanos: u32) -> Result<()> {
    if mtime_unix <= 0 {
        return Ok(());
    }
    let ft = FileTime::from_unix_time(mtime_unix, mtime_nanos);
    set_file_mtime(path, ft).with_context(|| {
        format!(
            "versioned::restore_mtime failed to set mtime for {:?}",
            path
        )
    })?;
    Ok(())
}
