use anyhow::{Context, Result};
use backup_core::config::model::RuntimeTuning;
use backup_core::io::{run_with_policy, BlockingIoPolicy, CancellationFlag};
use chrono::Utc;
use std::collections::hash_map::RandomState;
use std::fs::{self, File, OpenOptions};
use std::hash::{BuildHasher, Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::warn;

const EXCLUSIVE_CREATE_ATTEMPTS: usize = 32;
static EXPORT_FILE_SEQ: AtomicU64 = AtomicU64::new(0);

struct IncompleteFileGuard {
    path: PathBuf,
    committed: bool,
}

impl IncompleteFileGuard {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            committed: false,
        }
    }

    fn commit(&mut self) {
        self.committed = true;
    }
}

impl Drop for IncompleteFileGuard {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn create_exclusive_export(dir: &Path, stem: &str) -> std::io::Result<(PathBuf, File)> {
    let random_state = RandomState::new();
    for _ in 0..EXCLUSIVE_CREATE_ATTEMPTS {
        let mut hasher = random_state.build_hasher();
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .hash(&mut hasher);
        std::process::id().hash(&mut hasher);
        EXPORT_FILE_SEQ
            .fetch_add(1, Ordering::Relaxed)
            .hash(&mut hasher);
        let path = dir.join(format!("{stem}-{:016x}.txt", hasher.finish()));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not allocate a unique log export filename",
    ))
}

fn write_exclusive_export_with<F>(dir: &Path, stem: &str, mut write: F) -> Result<PathBuf>
where
    F: FnMut(&mut File) -> std::io::Result<()>,
{
    let (path, mut file) = create_exclusive_export(dir, stem)
        .context("failed to create a new log export in the selected directory")?;
    let mut guard = IncompleteFileGuard::new(path.clone());
    write(&mut file).context("failed to write the new log export")?;
    file.sync_all()
        .context("failed to finish the new log export")?;
    guard.commit();
    Ok(path)
}

/// Centralize log path resolution for GUI features.
pub fn log_path() -> Result<PathBuf> {
    backup_core::platform::paths::log_file_path()
        .context("gui::api::logs_api::log_path failed to resolve log path")
}

/// Provide quick access to recent daemon logs.
pub fn read_log_tail(limit: Option<usize>) -> Result<String> {
    let path = log_path()?;
    let runtime = resolve_runtime_tuning();
    let max_lines = resolve_tail_limit(limit, &runtime);
    let tail = match read_tail_from_file(
        &path,
        max_lines,
        runtime.log_tail_read_chunk_bytes,
        runtime.log_tail_max_bytes,
    ) {
        Ok(tail) => tail,
        Err(e) if is_not_found(&e) => {
            warn!(
                "gui::api::logs_api::read_log_tail log file not found at {:?}",
                path
            );
            String::new()
        }
        Err(e) => {
            return Err(anyhow::anyhow!(
                "gui::api::logs_api::read_log_tail failed to read log file {:?}: {}",
                path,
                e
            ));
        }
    };
    Ok(filter_hidden_log_lines(&tail))
}

/// Allow users to share logs for support.
pub fn export_logs(dest_dir: &Path) -> Result<PathBuf> {
    let io_policy = BlockingIoPolicy::bootstrap_defaults();
    let path = log_path()?;
    let data = run_with_policy(
        "gui::api::logs_api::export_logs read log file",
        &io_policy,
        CancellationFlag::none(),
        || {
            std::fs::read_to_string(&path).with_context(|| {
                format!(
                    "gui::api::logs_api::export_logs failed to read log file {:?}",
                    path
                )
            })
        },
    )?;
    let data = filter_hidden_log_lines(&data);
    let stem = format!("BackupSync-logs-{}", Utc::now().format("%Y%m%d-%H%M%S"));
    let dest = run_with_policy(
        "gui::api::logs_api::export_logs write export file",
        &io_policy,
        CancellationFlag::none(),
        || write_exclusive_export_with(dest_dir, &stem, |file| file.write_all(data.as_bytes())),
    )?;
    Ok(dest)
}

/// Centralize log tail limit selection with clear fallback logging.
fn resolve_runtime_tuning() -> RuntimeTuning {
    match backup_core::load_validated_config() {
        Ok(cfg) => cfg.runtime,
        Err(error) => {
            warn!(
                error = %error,
                "gui::api::logs_api::resolve_runtime_tuning failed to load config; using runtime defaults"
            );
            RuntimeTuning::default()
        }
    }
}

/// Keep log tail policy selection centralized and free of magic values.
fn resolve_tail_limit(limit: Option<usize>, runtime: &RuntimeTuning) -> usize {
    if let Some(limit) = limit {
        return limit;
    }
    runtime.log_tail_lines
}

/// Avoid sending large logs when only recent lines are needed.
fn tail_lines(data: &str, max_lines: usize) -> String {
    if max_lines == 0 {
        return String::new();
    }
    let lines: Vec<&str> = data.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
}

/// Keep `read_log_tail` behavior stable when logs are not present yet.
fn is_not_found(err: &anyhow::Error) -> bool {
    err.downcast_ref::<std::io::Error>()
        .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound)
}

/// Avoid large allocations when logs grow over time.
fn read_tail_from_file(
    path: &Path,
    max_lines: usize,
    read_chunk_bytes: usize,
    max_bytes: u64,
) -> Result<String> {
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};

    if max_lines == 0 {
        return Ok(String::new());
    }
    let io_policy = BlockingIoPolicy::bootstrap_defaults();
    run_with_policy(
        "gui::api::logs_api::read_tail_from_file read tail",
        &io_policy,
        CancellationFlag::none(),
        || {
            let mut f = File::open(path).with_context(|| {
                format!(
                    "gui::api::logs_api::read_tail_from_file failed to open {:?}",
                    path
                )
            })?;
            let mut pos = f
                .metadata()
                .with_context(|| {
                    format!(
                        "gui::api::logs_api::read_tail_from_file failed to stat {:?}",
                        path
                    )
                })?
                .len();

            let mut chunks: Vec<Vec<u8>> = Vec::new();
            let mut newline_count: usize = 0;
            let mut total: u64 = 0;
            while pos > 0 && newline_count <= max_lines && total < max_bytes {
                let take = std::cmp::min(read_chunk_bytes.max(1) as u64, pos) as usize;
                pos -= take as u64;
                f.seek(SeekFrom::Start(pos)).with_context(|| {
                    format!(
                        "gui::api::logs_api::read_tail_from_file failed to seek {:?}",
                        path
                    )
                })?;
                let mut buf = vec![0u8; take];
                f.read_exact(&mut buf).with_context(|| {
                    format!(
                        "gui::api::logs_api::read_tail_from_file failed to read {:?}",
                        path
                    )
                })?;
                newline_count += buf.iter().filter(|b| **b == b'\n').count();
                total += buf.len() as u64;
                chunks.push(buf);
            }

            chunks.reverse();
            let mut bytes = Vec::with_capacity(total as usize);
            for c in chunks {
                bytes.extend_from_slice(&c);
            }

            let s = String::from_utf8_lossy(&bytes).to_string();
            Ok(tail_lines(&s, max_lines))
        },
    )
}

/// Ensure removed authentication flows do not linger in UI log views or exports.
fn filter_hidden_log_lines(data: &str) -> String {
    const NEEDLES: [&str; 2] = ["AUTH_BYPASS", "allowing without unlock"];
    if !NEEDLES.iter().any(|needle| data.contains(needle)) {
        return data.to_string();
    }
    let mut out = String::with_capacity(data.len());
    for line in data.lines() {
        if NEEDLES.iter().any(|needle| line.contains(needle)) {
            continue;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{filter_hidden_log_lines, write_exclusive_export_with};
    use std::fs;

    fn test_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "backup-sync-logs-api-{name}-{}-{}",
            std::process::id(),
            super::EXPORT_FILE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&dir).expect("create test directory");
        dir
    }

    #[test]
    fn filter_hidden_log_lines_removes_legacy_auth_lines() {
        let input = "\
2026-02-04T00:00:00Z INFO normal\n\
S allowing without unlock\n\
[cid=save-miqzosxg-e476] AUTH_BYPASS allowing without unlock\n\
2026-02-04T00:00:01Z INFO still here\n";
        let filtered = filter_hidden_log_lines(input);
        assert!(filtered.contains("INFO normal"));
        assert!(filtered.contains("INFO still here"));
        assert!(!filtered.contains("AUTH_BYPASS"));
        assert!(!filtered.contains("allowing without unlock"));
    }

    #[test]
    fn exclusive_export_preserves_predictable_existing_file() {
        let dir = test_dir("collision");
        let predictable = dir.join("BackupSync-logs-fixed.txt");
        fs::write(&predictable, b"keep me").expect("seed predictable file");

        let exported = write_exclusive_export_with(&dir, "BackupSync-logs-fixed", |file| {
            std::io::Write::write_all(file, b"new export")
        })
        .expect("write exclusive export");

        assert_ne!(exported, predictable);
        assert_eq!(fs::read(&predictable).expect("read seed"), b"keep me");
        assert_eq!(fs::read(&exported).expect("read export"), b"new export");
        fs::remove_dir_all(dir).expect("remove test directory");
    }

    #[test]
    fn failed_export_removes_partial_output() {
        let dir = test_dir("cleanup");
        let error = write_exclusive_export_with(&dir, "BackupSync-logs-fixed", |file| {
            std::io::Write::write_all(file, b"partial")?;
            Err(std::io::Error::other("injected write failure"))
        })
        .expect_err("export must fail");

        assert!(error.to_string().contains("failed to write"));
        assert_eq!(fs::read_dir(&dir).expect("read test directory").count(), 0);
        fs::remove_dir_all(dir).expect("remove test directory");
    }

    #[cfg(unix)]
    #[test]
    fn exclusive_export_does_not_follow_predictable_symlink() {
        use std::os::unix::fs::symlink;

        let dir = test_dir("symlink");
        let target = dir.join("target.txt");
        let predictable = dir.join("BackupSync-logs-fixed.txt");
        fs::write(&target, b"target contents").expect("seed target");
        symlink(&target, &predictable).expect("create symlink");

        let exported = write_exclusive_export_with(&dir, "BackupSync-logs-fixed", |file| {
            std::io::Write::write_all(file, b"new export")
        })
        .expect("write exclusive export");

        assert_ne!(exported, predictable);
        assert_eq!(fs::read(&target).expect("read target"), b"target contents");
        fs::remove_dir_all(dir).expect("remove test directory");
    }
}
