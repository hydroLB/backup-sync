use super::model::{Manifest, ManifestEntry, ManifestEntryKind, VersionIndex, VersionInfo};
use crate::config::model::{Config, Destination, WatchedKind, WatchedPath};
use crate::logging::redact_path;
use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

pub struct FolderDescriptor {
    pub source_path: PathBuf,
    pub destination_root: PathBuf,
    pub keep_versions: usize,
}

#[derive(Debug, Default, Clone)]
pub struct FolderBackupResult {
    pub changed: bool,
    pub version_id: Option<String>,
    pub blobs_written: usize,
    pub bytes_written: u64,
}

#[derive(Debug, Default, Clone)]
pub struct BackupCycleResult {
    pub folders_scanned: usize,
    pub versions_created: usize,
    pub blobs_written: usize,
    pub bytes_written: u64,
}

pub(crate) const STORE_DIR: &str = ".backup_sync";
pub(crate) const STORE_SCHEMA_VERSION: u32 = 1;

pub fn run_backup_cycle(cfg: &Config) -> Result<BackupCycleResult> {
    let destinations_by_id: HashMap<&str, &Destination> = cfg
        .destinations
        .iter()
        .map(|d| (d.id.as_str(), d))
        .collect();

    let mut cycle = BackupCycleResult::default();
    for watched in cfg.watched.iter().filter(|w| w.enabled) {
        let dest = destinations_by_id
            .get(watched.destination_id.as_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "versioned::run_backup_cycle missing destination id {} for watched path {}",
                    watched.destination_id,
                    redact_path(&watched.path)
                )
            })?;
        let keep_versions = watched
            .max_backups_per_file
            .or(dest.max_backups_per_file)
            .unwrap_or(cfg.max_backups_per_file);
        let descriptor = FolderDescriptor {
            source_path: watched.path.clone(),
            destination_root: dest.path.clone(),
            keep_versions,
        };
        let result = backup_one_folder(cfg, watched, &descriptor)?;
        cycle.folders_scanned += 1;
        if result.changed {
            cycle.versions_created += 1;
        }
        cycle.blobs_written += result.blobs_written;
        cycle.bytes_written += result.bytes_written;
    }
    Ok(cycle)
}

fn backup_one_folder(
    cfg: &Config,
    watched: &WatchedPath,
    desc: &FolderDescriptor,
) -> Result<FolderBackupResult> {
    let store_root = store_root(&desc.destination_root);
    let blobs_root = blobs_root(&store_root);
    fs::create_dir_all(&blobs_root).with_context(|| {
        format!(
            "versioned::backup_one_folder failed to create blob directory {:?}",
            blobs_root
        )
    })?;

    let source_id = sha256_hex(desc.source_path.to_string_lossy().as_bytes());
    let source_root = sources_root(&store_root).join(&source_id);
    let manifests_root = source_root.join("manifests");
    fs::create_dir_all(&manifests_root).with_context(|| {
        format!(
            "versioned::backup_one_folder failed to create manifests directory {:?}",
            manifests_root
        )
    })?;

    let mut index = load_index(&source_root, &desc.source_path)?;
    let prev = latest_manifest(&manifests_root, &index)?;
    let snapshot = scan_snapshot(cfg, watched, &prev)?;
    let changed = match &prev {
        None => true,
        Some(prev_manifest) => !manifests_equivalent(prev_manifest, &snapshot),
    };
    if !changed {
        return Ok(FolderBackupResult {
            changed: false,
            ..Default::default()
        });
    }

    let mut version_id = chrono::Utc::now().format("%Y%m%d-%H%M%S-%f").to_string();
    let mut manifest_path = manifests_root.join(format!("{version_id}.json"));
    let mut collision = 0u32;
    while manifest_path.exists() {
        collision += 1;
        version_id = format!("{}-{}", version_id, collision);
        manifest_path = manifests_root.join(format!("{version_id}.json"));
    }

    let mut written = FolderBackupResult {
        changed: true,
        version_id: Some(version_id.clone()),
        ..Default::default()
    };

    for entry in snapshot.entries.values() {
        if entry.kind != ManifestEntryKind::File {
            continue;
        }
        let hash = entry
            .sha256
            .as_ref()
            .context("versioned::backup_one_folder missing sha256 for file entry")?;
        let blob_path = blob_path(&blobs_root, hash);
        if blob_path.exists() {
            continue;
        }
        let src_path = match watched.kind {
            WatchedKind::File => desc.source_path.clone(),
            WatchedKind::Directory => desc.source_path.join(Path::new(&entry.rel_path)),
        };
        let (count, bytes) = write_blob(&src_path, &blob_path, cfg.hashing.timeout_seconds)?;
        written.blobs_written += count;
        written.bytes_written += bytes;
    }

    write_json_atomic(&manifest_path, &snapshot).with_context(|| {
        format!(
            "versioned::backup_one_folder failed to write manifest {:?}",
            manifest_path
        )
    })?;

    index.schema_version = STORE_SCHEMA_VERSION;
    index.source_path = desc.source_path.to_string_lossy().into_owned();
    index.versions.push(VersionInfo {
        id: version_id.clone(),
        created_at_unix: snapshot.created_at_unix,
    });
    index.versions.sort_by(|a, b| {
        a.created_at_unix
            .cmp(&b.created_at_unix)
            .then_with(|| a.id.cmp(&b.id))
    });

    let mut deleted_any = false;
    while index.versions.len() > desc.keep_versions {
        if let Some(oldest) = index.versions.first().cloned() {
            let path = manifests_root.join(format!("{}.json", oldest.id));
            let _ = fs::remove_file(&path);
            index.versions.remove(0);
            deleted_any = true;
        } else {
            break;
        }
    }

    write_index(&source_root, &index)?;

    if deleted_any {
        gc_unreferenced_blobs(&store_root, &blobs_root)?;
    }

    Ok(written)
}

pub(crate) fn store_root(destination_root: &Path) -> PathBuf {
    destination_root
        .join(STORE_DIR)
        .join(format!("v{STORE_SCHEMA_VERSION}"))
}

pub(crate) fn sources_root(store_root: &Path) -> PathBuf {
    store_root.join("sources")
}

pub(crate) fn blobs_root(store_root: &Path) -> PathBuf {
    store_root.join("blobs").join("sha256")
}

pub(crate) fn blob_path(blobs_root: &Path, hash: &str) -> PathBuf {
    let prefix = &hash[0..2];
    blobs_root.join(prefix).join(hash)
}

fn write_blob(src_path: &Path, blob_path: &Path, timeout_seconds: u64) -> Result<(usize, u64)> {
    if let Some(parent) = blob_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "versioned::write_blob failed to create blob parent directory {:?}",
                parent
            )
        })?;
    }
    let start = Instant::now();
    let mut file = fs::File::open(src_path).with_context(|| {
        format!(
            "versioned::write_blob failed to open source file {:?}",
            src_path
        )
    })?;
    let mut temp = tempfile::NamedTempFile::new_in(
        blob_path
            .parent()
            .context("versioned::write_blob missing blob parent")?,
    )
    .context("versioned::write_blob failed to create temp file")?;
    let mut buf = vec![0u8; 64 * 1024];
    let mut written: u64 = 0;
    loop {
        if timeout_seconds > 0 && start.elapsed().as_secs() > timeout_seconds {
            anyhow::bail!(
                "versioned::write_blob timed out after {}s writing {:?}",
                timeout_seconds,
                src_path
            );
        }
        let n = file
            .read(&mut buf)
            .with_context(|| format!("versioned::write_blob failed reading {:?}", src_path))?;
        if n == 0 {
            break;
        }
        temp.write_all(&buf[..n]).with_context(|| {
            format!(
                "versioned::write_blob failed writing temp for {:?}",
                src_path
            )
        })?;
        written += n as u64;
    }
    temp.flush().ok();
    temp.persist(blob_path).map_err(|e| {
        anyhow::anyhow!(
            "versioned::write_blob failed to persist blob {:?}: {}",
            blob_path,
            e
        )
    })?;
    Ok((1, written))
}

fn scan_snapshot(cfg: &Config, watched: &WatchedPath, prev: &Option<Manifest>) -> Result<Manifest> {
    let prev_entries: HashMap<&str, &ManifestEntry> = prev
        .as_ref()
        .map(|m| {
            m.entries
                .values()
                .map(|e| (e.rel_path.as_str(), e))
                .collect()
        })
        .unwrap_or_default();

    let ignore = build_ignore_set(&cfg.ignore_patterns)
        .context("versioned::scan_snapshot failed to build ignore set")?;

    let mut entries: BTreeMap<String, ManifestEntry> = BTreeMap::new();

    match watched.kind {
        WatchedKind::File => {
            let name = watched
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "file".to_string());
            let e = scan_one_file(
                &watched.path,
                &name,
                cfg.hashing.timeout_seconds,
                &prev_entries,
            )?;
            entries.insert(name.clone(), e);
        }
        WatchedKind::Directory => {
            for item in walkdir::WalkDir::new(&watched.path)
                .follow_links(false)
                .into_iter()
            {
                let item = item.with_context(|| {
                    format!("versioned::scan_snapshot failed walking {:?}", watched.path)
                })?;
                let path = item.path();
                let rel = path.strip_prefix(&watched.path).unwrap_or(path);
                if rel.as_os_str().is_empty() {
                    continue;
                }
                if cfg.skip_hidden && is_hidden(rel) {
                    continue;
                }
                if ignore.is_match(rel) {
                    continue;
                }
                if item.file_type().is_dir() {
                    let rel_str = normalize_rel(rel);
                    entries.insert(
                        rel_str.clone(),
                        ManifestEntry {
                            kind: ManifestEntryKind::Dir,
                            rel_path: rel_str,
                            len: 0,
                            mtime_unix: 0,
                            mtime_nanos: 0,
                            sha256: None,
                        },
                    );
                    continue;
                }
                if item.file_type().is_file() {
                    let rel_str = normalize_rel(rel);
                    let entry =
                        scan_one_file(path, &rel_str, cfg.hashing.timeout_seconds, &prev_entries)?;
                    entries.insert(rel_str.clone(), entry);
                }
            }
        }
    }

    Ok(Manifest {
        schema_version: STORE_SCHEMA_VERSION,
        source_path: watched.path.to_string_lossy().into_owned(),
        created_at_unix: chrono::Utc::now().timestamp(),
        entries,
    })
}

fn scan_one_file(
    abs_path: &Path,
    rel_path: &str,
    timeout_seconds: u64,
    prev: &HashMap<&str, &ManifestEntry>,
) -> Result<ManifestEntry> {
    let meta = fs::metadata(abs_path).with_context(|| {
        format!(
            "versioned::scan_one_file failed to stat file {:?}",
            abs_path
        )
    })?;
    let (mtime_unix, mtime_nanos) = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| (d.as_secs() as i64, d.subsec_nanos()))
        .unwrap_or((0, 0));
    let len = meta.len();
    if let Some(prev) = prev.get(rel_path) {
        if prev.kind == ManifestEntryKind::File
            && prev.len == len
            && prev.mtime_unix == mtime_unix
            && prev.mtime_nanos == mtime_nanos
            && prev.sha256.is_some()
        {
            return Ok(ManifestEntry {
                kind: ManifestEntryKind::File,
                rel_path: rel_path.to_string(),
                len,
                mtime_unix,
                mtime_nanos,
                sha256: prev.sha256.clone(),
            });
        }
    }
    let hash = sha256_file(abs_path, timeout_seconds)?;
    Ok(ManifestEntry {
        kind: ManifestEntryKind::File,
        rel_path: rel_path.to_string(),
        len,
        mtime_unix,
        mtime_nanos,
        sha256: Some(hash),
    })
}

fn manifests_equivalent(prev: &Manifest, next: &Manifest) -> bool {
    if prev.entries.len() != next.entries.len() {
        return false;
    }
    for (k, v) in next.entries.iter() {
        let prev_e = match prev.entries.get(k) {
            None => return false,
            Some(e) => e,
        };
        if prev_e.kind != v.kind {
            return false;
        }
        if v.kind == ManifestEntryKind::File && prev_e.sha256 != v.sha256 {
            return false;
        }
    }
    true
}

fn load_index(source_root: &Path, source_path: &Path) -> Result<VersionIndex> {
    let path = source_root.join("index.json");
    if !path.exists() {
        return Ok(VersionIndex {
            schema_version: STORE_SCHEMA_VERSION,
            source_path: source_path.to_string_lossy().into_owned(),
            versions: Vec::new(),
        });
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("versioned::load_index failed to read {:?}", path))?;
    let index: VersionIndex = serde_json::from_str(&raw)
        .with_context(|| format!("versioned::load_index failed to parse {:?}", path))?;
    Ok(index)
}

fn write_index(source_root: &Path, index: &VersionIndex) -> Result<()> {
    let path = source_root.join("index.json");
    write_json_atomic(&path, index).with_context(|| {
        format!(
            "versioned::write_index failed to write version index {:?}",
            path
        )
    })
}

fn latest_manifest(manifests_root: &Path, index: &VersionIndex) -> Result<Option<Manifest>> {
    let last = match index.versions.last() {
        None => return Ok(None),
        Some(v) => v,
    };
    let path = manifests_root.join(format!("{}.json", last.id));
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("versioned::latest_manifest failed to read {:?}", path))?;
    let manifest: Manifest = serde_json::from_str(&raw)
        .with_context(|| format!("versioned::latest_manifest failed to parse {:?}", path))?;
    Ok(Some(manifest))
}

fn write_json_atomic<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "versioned::write_json_atomic failed to create parent {:?}",
                parent
            )
        })?;
    }
    let tmp = path.with_extension("tmp");
    let raw = serde_json::to_string_pretty(value)
        .context("versioned::write_json_atomic failed to serialize JSON")?;
    fs::write(&tmp, raw).with_context(|| {
        format!(
            "versioned::write_json_atomic failed to write temp file {:?}",
            tmp
        )
    })?;
    fs::rename(&tmp, path).with_context(|| {
        format!(
            "versioned::write_json_atomic failed to rename {:?} -> {:?}",
            tmp, path
        )
    })?;
    Ok(())
}

fn sha256_file(path: &Path, timeout_seconds: u64) -> Result<String> {
    let start = Instant::now();
    let mut file = fs::File::open(path)
        .with_context(|| format!("versioned::sha256_file failed to open {:?}", path))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        if timeout_seconds > 0 && start.elapsed().as_secs() > timeout_seconds {
            anyhow::bail!(
                "versioned::sha256_file timed out after {}s hashing {:?}",
                timeout_seconds,
                path
            );
        }
        let n = file
            .read(&mut buf)
            .with_context(|| format!("versioned::sha256_file failed reading {:?}", path))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(sha256_hex(&hasher.finalize()))
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{:02x}", b);
    }
    out
}

pub(crate) fn normalize_rel(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    if s.starts_with("./") {
        s.trim_start_matches("./").to_string()
    } else {
        s
    }
}

fn is_hidden(rel: &Path) -> bool {
    rel.components().any(|c| {
        let name = c.as_os_str().to_string_lossy();
        name.starts_with('.')
    })
}

fn build_ignore_set(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for p in patterns {
        if let Ok(g) = Glob::new(p) {
            let _ = builder.add(g);
        }
    }
    builder
        .build()
        .context("versioned::build_ignore_set failed to build ignore set")
}

fn gc_unreferenced_blobs(store_root: &Path, blobs_root: &Path) -> Result<()> {
    let sources = sources_root(store_root);
    if !sources.exists() {
        return Ok(());
    }
    let mut referenced: HashSet<String> = HashSet::new();
    for src in fs::read_dir(&sources).with_context(|| {
        format!(
            "versioned::gc_unreferenced_blobs failed to read {:?}",
            sources
        )
    })? {
        let src = src?;
        let manifests = src.path().join("manifests");
        if !manifests.exists() {
            continue;
        }
        for entry in fs::read_dir(&manifests)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let raw = fs::read_to_string(&path).with_context(|| {
                format!("versioned::gc_unreferenced_blobs failed to read {:?}", path)
            })?;
            let manifest: Manifest = serde_json::from_str(&raw).with_context(|| {
                format!(
                    "versioned::gc_unreferenced_blobs failed to parse {:?}",
                    path
                )
            })?;
            for e in manifest.entries.values() {
                if let Some(h) = e.sha256.as_ref() {
                    referenced.insert(h.clone());
                }
            }
        }
    }

    if !blobs_root.exists() {
        return Ok(());
    }
    for prefix in fs::read_dir(blobs_root).with_context(|| {
        format!(
            "versioned::gc_unreferenced_blobs failed to read {:?}",
            blobs_root
        )
    })? {
        let prefix = prefix?;
        if !prefix.file_type()?.is_dir() {
            continue;
        }
        for blob in fs::read_dir(prefix.path())? {
            let blob = blob?;
            let path = blob.path();
            if !blob.file_type()?.is_file() {
                continue;
            }
            let name = match path.file_name().and_then(|n| n.to_str()) {
                None => continue,
                Some(n) => n,
            };
            if !referenced.contains(name) {
                let _ = fs::remove_file(&path);
            }
        }
    }
    Ok(())
}
