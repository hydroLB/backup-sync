use crate::commands::{correlation, error::ErrorEnvelope, io_policy::run_blocking_io};
use anyhow::Context;
use backup_core::{
    backup::versioned,
    load_validated_config,
    logging::{redact_path, redact_text},
    platform::paths,
    state::store::StateStore,
};
use chrono::Utc;
use dirs::desktop_dir;
use serde::Serialize;
use std::collections::hash_map::RandomState;
use std::fs::{self, File, OpenOptions};
use std::hash::{BuildHasher, Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

const EXCLUSIVE_CREATE_ATTEMPTS: usize = 32;
static HEALTH_FILE_SEQ: AtomicU64 = AtomicU64::new(0);

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

fn create_exclusive_health_report(dir: &Path, stem: &str) -> std::io::Result<(PathBuf, File)> {
    let random_state = RandomState::new();
    for _ in 0..EXCLUSIVE_CREATE_ATTEMPTS {
        let mut hasher = random_state.build_hasher();
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .hash(&mut hasher);
        std::process::id().hash(&mut hasher);
        HEALTH_FILE_SEQ
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
        "could not allocate a unique health report filename",
    ))
}

fn write_exclusive_health_report_with<F>(
    dir: &Path,
    stem: &str,
    mut write: F,
) -> anyhow::Result<PathBuf>
where
    F: FnMut(&mut File) -> std::io::Result<()>,
{
    let (path, mut file) = create_exclusive_health_report(dir, stem)
        .context("failed to create a new health report on the Desktop")?;
    let mut guard = IncompleteFileGuard::new(path.clone());
    write(&mut file).context("failed to write the new health report")?;
    file.sync_all()
        .context("failed to finish the new health report")?;
    guard.commit();
    Ok(path)
}

#[derive(Serialize)]
/// Show verification status in the UI.
pub struct VerifyResult {
    pub ok: usize,
    pub bad: usize,
    pub last_verify_ts: Option<i64>,
    pub last_verify_status: Option<String>,
    pub last_verify_issues: Option<usize>,
}

#[tauri::command]
/// Allow users to verify backup integrity on demand.
pub async fn verify_cmd(correlation_id: Option<String>) -> Result<VerifyResult, ErrorEnvelope> {
    let cid = correlation::cid("verify", correlation_id);
    let task_cid = cid.clone();
    tokio::task::spawn_blocking(move || verify_blocking(task_cid))
        .await
        .map_err(|error| blocking_task_error(&cid, "verify_cmd", error))?
}

fn verify_blocking(cid: String) -> Result<VerifyResult, ErrorEnvelope> {
    info!(cid = %cid, action = "verify_start", "gui verify requested");
    let cfg = load_validated_config().map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_LOAD",
            format!(
                "[cid={}] backup::verify::verify_cmd failed to load config: {}",
                cid, e
            ),
        )
    })?;
    let state_path = paths::state_file_path().map_err(|e| {
        ErrorEnvelope::new(
            "STATE_PATH",
            format!(
                "backup::verify::verify_cmd failed to resolve state path: {}",
                e
            ),
        )
    })?;
    let (mut state, store) = StateStore::load_or_default(state_path).map_err(|e| {
        ErrorEnvelope::new(
            "STATE_LOAD",
            format!("backup::verify::verify_cmd failed to load state: {}", e),
        )
    })?;
    let now = Utc::now().timestamp();
    let res = versioned::scrub_versioned_store(
        &cfg,
        &cfg.hashing,
        versioned::ScrubMode::Full,
        cfg.runtime.scrub_sample_blobs,
        cfg.runtime.scrub_sample_versions_per_source,
        now as u64,
    )
    .map_err(|e| {
        ErrorEnvelope::new(
            "VERIFY_FAILED",
            format!("[cid={}] backup::verify::verify_cmd failed: {}", cid, e),
        )
    })?;
    let bad = res.hash_mismatches + res.missing_blobs + res.manifests_bad;
    let ok = res.blobs_hashed.saturating_sub(res.hash_mismatches);
    state.last_verify_ts = Some(now);
    state.last_verify_issues = Some(bad);
    state.last_verify_status = Some(if bad == 0 {
        "ok (full)".into()
    } else {
        "issues_detected (full)".into()
    });
    state.last_scrub_full_ts = Some(now);
    store.persist(&state).map_err(|e| {
        ErrorEnvelope::new(
            "STATE_SAVE",
            format!(
                "[cid={}] backup::verify::verify_cmd failed to save state: {}",
                cid, e
            ),
        )
    })?;
    info!(
        cid = %cid,
        action = "verify_complete",
        ok = ok,
        bad = bad,
        "gui verify completed"
    );
    Ok(VerifyResult {
        ok,
        bad,
        last_verify_ts: state.last_verify_ts,
        last_verify_status: state.last_verify_status.clone(),
        last_verify_issues: state.last_verify_issues,
    })
}

#[tauri::command]
/// Allow users to share health summaries.
pub async fn export_health_report_cmd(
    correlation_id: Option<String>,
) -> Result<String, ErrorEnvelope> {
    let cid = correlation::cid("health", correlation_id);
    info!(
        cid = %cid,
        action = "export_health_start",
        "gui health report export requested"
    );
    let status = fetch_status_snapshot().await;
    let task_cid = cid.clone();
    tokio::task::spawn_blocking(move || export_health_report_blocking(task_cid, status))
        .await
        .map_err(|error| blocking_task_error(&cid, "export_health_report_cmd", error))?
}

fn export_health_report_blocking(cid: String, status: String) -> Result<String, ErrorEnvelope> {
    let cfg = load_validated_config().map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_LOAD",
            format!(
                "[cid={}] export_health_report_cmd failed to load config: {}",
                cid, e
            ),
        )
    })?;
    let state_path = paths::state_file_path().map_err(|e| {
        ErrorEnvelope::new(
            "STATE_PATH",
            format!(
                "[cid={}] export_health_report_cmd failed to resolve state path: {}",
                cid, e
            ),
        )
    })?;
    let (state, _) = StateStore::load_or_default(state_path.clone()).map_err(|e| {
        ErrorEnvelope::new(
            "STATE_LOAD",
            format!(
                "[cid={}] export_health_report_cmd failed to load state: {}",
                cid, e
            ),
        )
    })?;
    let report = build_health_report(&cfg, &state, &state_path, &status);
    let dest = write_health_report(&report).map_err(|e| {
        ErrorEnvelope::new(
            "HEALTH_WRITE",
            format!(
                "[cid={}] export_health_report_cmd failed to write report: {}",
                cid,
                redact_text(&e.to_string())
            ),
        )
    })?;
    info!(
        cid = %cid,
        action = "export_health_complete",
        report_path = %redact_path(&dest),
        "gui health report export completed"
    );
    Ok(dest.display().to_string())
}

fn blocking_task_error(cid: &str, command: &str, error: tokio::task::JoinError) -> ErrorEnvelope {
    ErrorEnvelope::new(
        "BLOCKING_TASK_FAILED",
        format!("[cid={cid}] {command} blocking task failed: {error}"),
    )
}

/// Include current daemon status in diagnostics.
async fn fetch_status_snapshot() -> String {
    match super::super::status::get_status().await {
        Ok(status) => match serde_json::to_string_pretty(&status) {
            Ok(body) => body,
            Err(e) => format!(
                "backup::verify::fetch_status_snapshot failed to serialize status: {}",
                e
            ),
        },
        Err(e) => format!(
            "backup::verify::fetch_status_snapshot status unavailable: {}",
            e.message
        ),
    }
}

/// Keep report formatting centralized.
fn build_health_report(
    cfg: &backup_core::Config,
    state: &backup_core::state::StoredState,
    state_path: &std::path::Path,
    status: &str,
) -> String {
    let config_path = match paths::config_file_path() {
        Ok(path) => path.display().to_string(),
        Err(e) => format!(
            "backup::verify::build_health_report config path unavailable: {}",
            e
        ),
    };
    let mut report = String::new();
    report.push_str("# Backup Sync Health Report\n");
    report.push_str(&format!("Generated: {}\n", Utc::now()));
    report.push_str(&format!(
        "Config path: {}\n",
        redact_path(std::path::Path::new(&config_path))
    ));
    report.push_str(&format!("State path: {}\n", redact_path(state_path)));
    report.push_str(&format!(
        "Backup destination: {}\n",
        redact_path(&cfg.backup_root)
    ));
    report.push_str(&format!("Watched entries: {}\n", cfg.watched.len()));
    report.push_str(&format!(
        "Last error: {}\n",
        redact_text(&format!("{:?}", state.last_error))
    ));
    report.push_str("\nStatus snapshot:\n");
    report.push_str(&redact_text(status));
    report
}

/// Persist the report for sharing and diagnostics.
fn write_health_report(report: &str) -> anyhow::Result<PathBuf> {
    let dest_dir = desktop_dir().ok_or_else(|| {
        anyhow::anyhow!("No Desktop directory is available for the health report")
    })?;
    let stem = format!("BackupSync-health-{}", Utc::now().format("%Y%m%d-%H%M%S"));
    run_blocking_io(
        "gui::backup::verify::write_health_report write file",
        || {
            write_exclusive_health_report_with(&dest_dir, &stem, |file| {
                file.write_all(report.as_bytes())
            })
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{build_health_report, write_exclusive_health_report_with};
    use std::fs;

    fn test_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "backup-sync-health-report-{name}-{}-{}",
            std::process::id(),
            super::HEALTH_FILE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&dir).expect("create test directory");
        dir
    }

    #[test]
    fn health_report_uses_product_identity() {
        let cfg = backup_core::config::load::default_config().expect("build default config");
        let state = backup_core::state::StoredState::default();
        let report = build_health_report(&cfg, &state, std::path::Path::new("state.json"), "ok");
        assert!(report.starts_with("# Backup Sync Health Report\n"));
    }

    #[test]
    fn exclusive_health_report_preserves_predictable_existing_file() {
        let dir = test_dir("collision");
        let predictable = dir.join("BackupSync-health-fixed.txt");
        fs::write(&predictable, b"keep me").expect("seed predictable file");

        let exported =
            write_exclusive_health_report_with(&dir, "BackupSync-health-fixed", |file| {
                std::io::Write::write_all(file, b"new report")
            })
            .expect("write exclusive report");

        assert_ne!(exported, predictable);
        assert_eq!(fs::read(&predictable).expect("read seed"), b"keep me");
        assert_eq!(fs::read(&exported).expect("read report"), b"new report");
        fs::remove_dir_all(dir).expect("remove test directory");
    }

    #[test]
    fn failed_health_report_removes_partial_output() {
        let dir = test_dir("cleanup");
        let error = write_exclusive_health_report_with(&dir, "BackupSync-health-fixed", |file| {
            std::io::Write::write_all(file, b"partial")?;
            Err(std::io::Error::other("injected write failure"))
        })
        .expect_err("report must fail");

        assert!(error.to_string().contains("failed to write"));
        assert_eq!(fs::read_dir(&dir).expect("read test directory").count(), 0);
        fs::remove_dir_all(dir).expect("remove test directory");
    }

    #[cfg(unix)]
    #[test]
    fn exclusive_health_report_does_not_follow_predictable_symlink() {
        use std::os::unix::fs::symlink;

        let dir = test_dir("symlink");
        let target = dir.join("target.txt");
        let predictable = dir.join("BackupSync-health-fixed.txt");
        fs::write(&target, b"target contents").expect("seed target");
        symlink(&target, &predictable).expect("create symlink");

        let exported =
            write_exclusive_health_report_with(&dir, "BackupSync-health-fixed", |file| {
                std::io::Write::write_all(file, b"new report")
            })
            .expect("write exclusive report");

        assert_ne!(exported, predictable);
        assert_eq!(fs::read(&target).expect("read target"), b"target contents");
        fs::remove_dir_all(dir).expect("remove test directory");
    }
}
