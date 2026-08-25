use crate::commands::correlation;
use crate::commands::error::ErrorEnvelope;
use crate::commands::io_policy::run_blocking_io;
use crate::commands::status::get_status;
use anyhow::Context;
use backup_core::config::load::default_config;
use backup_core::{
    load_validated_config,
    logging::{redact_path, redact_text},
    platform::paths,
    state::store::StateStore,
};
use chrono::Utc;
use dirs::desktop_dir;
use fs2::free_space;
use serde::Serialize;
use serde_json::json;
use std::collections::hash_map::RandomState;
use std::fs::{self, File, OpenOptions};
use std::hash::{BuildHasher, Hash, Hasher};
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};
use zip::write::FileOptions;
use zip::ZipWriter;

const EXCLUSIVE_CREATE_ATTEMPTS: usize = 32;
static DIAGNOSTIC_FILE_SEQ: AtomicU64 = AtomicU64::new(0);

/// Create a hard-to-guess output name without ever replacing an existing directory entry.
fn create_exclusive_file(
    dir: &Path,
    stem: &str,
    extension: &str,
) -> std::io::Result<(PathBuf, File)> {
    let random_state = RandomState::new();
    for _ in 0..EXCLUSIVE_CREATE_ATTEMPTS {
        let mut hasher = random_state.build_hasher();
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .hash(&mut hasher);
        std::process::id().hash(&mut hasher);
        DIAGNOSTIC_FILE_SEQ
            .fetch_add(1, Ordering::Relaxed)
            .hash(&mut hasher);
        let path = dir.join(format!("{stem}-{:016x}.{extension}", hasher.finish()));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not allocate a unique diagnostics filename",
    ))
}

/// Remove an incomplete output unless its writer explicitly commits it.
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

#[tauri::command]
/// Provide a quick diagnostic snapshot for users.
pub async fn doctor_report_cmd(correlation_id: Option<String>) -> Result<String, ErrorEnvelope> {
    let cid = correlation::cid("doctor", correlation_id);
    info!(cid = %cid, action = "doctor_report_start", "gui doctor report requested");
    let cfg = load_validated_config().map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_LOAD",
            format!("support::doctor_report_cmd failed to load config: {}", e),
        )
    })?;
    let config_path = paths::config_file_path().map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_PATH",
            format!(
                "support::doctor_report_cmd failed to resolve config path: {}",
                e
            ),
        )
    })?;
    let state_path = paths::state_file_path().map_err(|e| {
        ErrorEnvelope::new(
            "STATE_PATH",
            format!(
                "support::doctor_report_cmd failed to resolve state path: {}",
                e
            ),
        )
    })?;
    let (state, _) = StateStore::load_or_default(state_path.clone()).map_err(|e| {
        ErrorEnvelope::new(
            "STATE_LOAD",
            format!("support::doctor_report_cmd failed to load state: {}", e),
        )
    })?;

    let report = build_doctor_report(&cfg, &state, &config_path, &state_path)?;

    let dest_dir = desktop_dir()
        .ok_or_else(|| ErrorEnvelope::new("NO_DESKTOP", "No desktop directory available"))?;
    let ts = Utc::now().format("%Y%m%d-%H%M%S");
    let stem = format!("BackupSync-doctor-{ts}");
    let dest = run_blocking_io("gui::support::doctor_report_cmd write report", || {
        let (dest, mut file) = create_exclusive_file(&dest_dir, &stem, "txt")
            .context("failed to reserve a unique report filename")?;
        let mut incomplete = IncompleteFileGuard::new(dest.clone());
        file.write_all(report.as_bytes())
            .context("failed to write the doctor report")?;
        file.sync_all().context("failed to flush the doctor report")?;
        incomplete.commit();
        Ok(dest)
    })
    .map_err(|e| {
        ErrorEnvelope::new(
            "DOCTOR_WRITE",
            format!(
                "Could not save the doctor report: {} Hint: confirm the Desktop exists and is writable, then retry.",
                redact_text(&e.to_string())
            ),
        )
    })?;
    info!(
        cid = %cid,
        action = "doctor_report_complete",
        report_path = %redact_path(&dest),
        "gui doctor report written"
    );
    Ok(dest.display().to_string())
}

/// Centralize report formatting.
fn build_doctor_report(
    cfg: &backup_core::Config,
    state: &backup_core::state::StoredState,
    config_path: &std::path::Path,
    state_path: &std::path::Path,
) -> Result<String, ErrorEnvelope> {
    let mut report = String::new();
    report.push_str("# Backup Sync Doctor Report\n");
    report.push_str(&format!("Generated: {}\n\n", Utc::now()));
    report.push_str(&format!("Config file: {}\n", redact_path(config_path)));
    report.push_str(&format!("State file: {}\n", redact_path(state_path)));
    let dest_path = cfg
        .destinations
        .first()
        .map(|d| d.path.clone())
        .unwrap_or_else(|| cfg.backup_root.clone());
    report.push_str(&format!(
        "Backup destination: {}\n",
        redact_path(&dest_path)
    ));
    report.push_str(&format!(
        "Destinations defined: {}\n",
        cfg.destinations.len()
    ));
    report.push_str(&format!("Watched entries: {}\n", cfg.watched.len()));
    append_missing_paths(cfg, &mut report);
    append_free_space(cfg, &dest_path, &mut report);
    append_write_probe(cfg, &dest_path, &mut report);
    report.push_str(&format!("Last error: {:?}\n", state.last_error));
    report.push_str(&format!(
        "Last verify: {:?} issues: {:?}\n",
        state.last_verify_ts, state.last_verify_issues
    ));
    Ok(report)
}

/// Surface missing paths in diagnostics.
fn append_missing_paths(cfg: &backup_core::Config, out: &mut String) {
    if cfg.watched.is_empty() {
        out.push_str("Problem: no watched paths configured.\n");
    }
    let (_existing, missing): (Vec<_>, Vec<_>) = cfg.watched.iter().partition(|w| w.path.exists());
    if !missing.is_empty() {
        out.push_str(&format!("Missing watched paths ({}):\n", missing.len()));
        for m in missing {
            out.push_str(&format!("  {}\n", redact_path(&m.path)));
        }
    }
}

/// Surface free space issues in diagnostics.
fn append_free_space(cfg: &backup_core::Config, dest_path: &std::path::Path, out: &mut String) {
    match free_space(dest_path) {
        Ok(bytes) => {
            out.push_str(&format!("Free space at destination: {} bytes\n", bytes));
            if let Some(min_free) = cfg.min_free_space_bytes {
                if bytes < min_free {
                    out.push_str(&format!(
                        "Problem: free space below configured minimum {}\n",
                        min_free
                    ));
                }
            }
        }
        Err(e) => out.push_str(&format!("Problem reading free space: {}\n", e)),
    }
}

/// Surface writability issues in diagnostics.
fn append_write_probe(_cfg: &backup_core::Config, dest_path: &std::path::Path, out: &mut String) {
    match write_probe(dest_path) {
        Ok(()) => out.push_str("Write check: OK\n"),
        Err(e) => out.push_str(&format!(
            "Problem: cannot write to backup destination: {}. Check destination permissions and available space.\n",
            redact_text(&e.to_string())
        )),
    }
}

fn write_probe(dest_path: &Path) -> anyhow::Result<()> {
    let stem = ".backup-sync-write-probe";
    let (probe, mut file) = run_blocking_io("gui::support::write_probe create probe", || {
        create_exclusive_file(dest_path, stem, "tmp")
            .context("failed to create an exclusive write probe")
    })?;
    let mut cleanup = IncompleteFileGuard::new(probe.clone());
    run_blocking_io("gui::support::write_probe write probe", || {
        file.write_all(b"probe")
            .context("failed to write the destination probe")?;
        file.sync_all()
            .context("failed to flush the destination probe")
    })?;
    drop(file);
    fs::remove_file(&probe).context("failed to remove the destination probe")?;
    cleanup.commit();
    Ok(())
}

#[derive(Serialize)]
/// Include config metadata without sensitive path details.
struct RedactedConfig {
    backup_root: String,
    watched_count: usize,
    interval_seconds: u64,
    max_parallel_copies: usize,
    max_backups_per_file: usize,
    max_bytes_per_second: Option<u64>,
    hash_buffer_bytes: usize,
    hash_timeout_seconds: u64,
    copy_timeout_seconds: u64,
    retry_jitter_pct: f64,
    ignore_patterns_count: usize,
    safe_mode: bool,
    destinations_count: usize,
    verify_interval_seconds: u64,
    prune_interval_cycles: u64,
    watcher_debounce_seconds: u64,
    ipc_timeout_seconds: u64,
    service_command_timeout_seconds: u64,
    service_command_retry_delay_ms: u64,
    service_command_poll_interval_ms: u64,
    tray_tooltip_refresh_seconds: u64,
    log_tail_lines: usize,
    simulation_sample_limit: usize,
    scan_capacity_multiplier: usize,
}

/// Share configuration metadata safely.
fn build_redacted(cfg: &backup_core::Config) -> RedactedConfig {
    RedactedConfig {
        backup_root: redact_path(&cfg.backup_root),
        watched_count: cfg.watched.len(),
        interval_seconds: cfg.interval_seconds,
        max_parallel_copies: cfg.max_parallel_copies,
        max_backups_per_file: cfg.max_backups_per_file,
        max_bytes_per_second: cfg.max_bytes_per_second,
        hash_buffer_bytes: cfg.hashing.buffer_bytes,
        hash_timeout_seconds: cfg.hashing.timeout_seconds,
        copy_timeout_seconds: cfg.execution.copy_timeout_seconds,
        retry_jitter_pct: cfg.execution.retry_jitter_pct,
        ignore_patterns_count: cfg.ignore_patterns.len(),
        safe_mode: cfg.safe_mode,
        destinations_count: cfg.destinations.len(),
        verify_interval_seconds: cfg.runtime.verify_interval_seconds,
        prune_interval_cycles: cfg.runtime.prune_interval_cycles,
        watcher_debounce_seconds: cfg.runtime.watcher_debounce_seconds,
        ipc_timeout_seconds: cfg.runtime.ipc_timeout_seconds,
        service_command_timeout_seconds: cfg.runtime.service_command_timeout_seconds,
        service_command_retry_delay_ms: cfg.runtime.service_command_retry_delay_ms,
        service_command_poll_interval_ms: cfg.runtime.service_command_poll_interval_ms,
        tray_tooltip_refresh_seconds: cfg.runtime.tray_tooltip_refresh_seconds,
        log_tail_lines: cfg.runtime.log_tail_lines,
        simulation_sample_limit: cfg.runtime.simulation_sample_limit,
        scan_capacity_multiplier: cfg.planning.scan_capacity_multiplier,
    }
}

/// Surface deviations from defaults in support bundles.
fn build_diff(cfg: &backup_core::Config) -> serde_json::Value {
    let defaults = match default_config() {
        Ok(defaults) => defaults,
        Err(e) => {
            let warn_cid = correlation::cid("diag", None);
            warn!(
                cid = %warn_cid,
                "support::build_diff failed to load default config, using current config: {}",
                e
            );
            cfg.clone()
        }
    };
    json!({
        "interval_seconds": (cfg.interval_seconds, defaults.interval_seconds),
        "max_backups_per_file": (cfg.max_backups_per_file, defaults.max_backups_per_file),
        "max_parallel_copies": (cfg.max_parallel_copies, defaults.max_parallel_copies),
        "max_bytes_per_second": (cfg.max_bytes_per_second, defaults.max_bytes_per_second),
        "hash_buffer_bytes": (cfg.hashing.buffer_bytes, defaults.hashing.buffer_bytes),
        "hash_timeout_seconds": (cfg.hashing.timeout_seconds, defaults.hashing.timeout_seconds),
        "copy_timeout_seconds": (cfg.execution.copy_timeout_seconds, defaults.execution.copy_timeout_seconds),
        "retry_jitter_pct": (cfg.execution.retry_jitter_pct, defaults.execution.retry_jitter_pct),
        "safe_mode": cfg.safe_mode,
        "ignore_patterns_count": cfg.ignore_patterns.len(),
        "watched_count": cfg.watched.len(),
        "verify_interval_seconds": (cfg.runtime.verify_interval_seconds, defaults.runtime.verify_interval_seconds),
        "prune_interval_cycles": (cfg.runtime.prune_interval_cycles, defaults.runtime.prune_interval_cycles),
        "watcher_debounce_seconds": (cfg.runtime.watcher_debounce_seconds, defaults.runtime.watcher_debounce_seconds),
        "ipc_timeout_seconds": (cfg.runtime.ipc_timeout_seconds, defaults.runtime.ipc_timeout_seconds),
        "service_command_timeout_seconds": (cfg.runtime.service_command_timeout_seconds, defaults.runtime.service_command_timeout_seconds),
        "service_command_retry_delay_ms": (cfg.runtime.service_command_retry_delay_ms, defaults.runtime.service_command_retry_delay_ms),
        "service_command_poll_interval_ms": (cfg.runtime.service_command_poll_interval_ms, defaults.runtime.service_command_poll_interval_ms),
        "tray_tooltip_refresh_seconds": (cfg.runtime.tray_tooltip_refresh_seconds, defaults.runtime.tray_tooltip_refresh_seconds),
        "log_tail_lines": (cfg.runtime.log_tail_lines, defaults.runtime.log_tail_lines),
        "simulation_sample_limit": (cfg.runtime.simulation_sample_limit, defaults.runtime.simulation_sample_limit),
        "scan_capacity_multiplier": (cfg.planning.scan_capacity_multiplier, defaults.planning.scan_capacity_multiplier),
    })
}

/// Include recent logs in support bundles.
fn tail_logs(path: &std::path::Path, max_lines: usize) -> String {
    let logs = match run_blocking_io("gui::support::tail_logs read log", || {
        std::fs::read_to_string(path)
            .with_context(|| format!("support::tail_logs failed to read {:?}", path))
    }) {
        Ok(data) => data,
        Err(e) => {
            return format!(
                "support::tail_logs failed to read log file {}: {}",
                redact_path(path),
                e
            );
        }
    };
    let tail = logs
        .lines()
        .rev()
        .take(max_lines)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n");
    redact_text(&tail)
}

/// Avoid silent serialization failures while keeping the bundle readable.
fn serialize_json<T: Serialize>(label: &str, value: &T) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(data) => data,
        Err(e) => format!("{} failed to serialize JSON: {}", label, e),
    }
}

/// Include recent activity in support bundles.
fn recent_activity_lines(state: &backup_core::state::StoredState) -> Vec<String> {
    state
        .recent_activity
        .iter()
        .rev()
        .take(20)
        .map(|a| {
            format!(
                "{} • {} bytes • {}",
                redact_path(std::path::Path::new(&a.path)),
                a.bytes,
                a.ts
            )
        })
        .collect()
}

/// Provide a human readable summary in the bundle.
fn build_primary_text(
    config_path: &std::path::Path,
    state_path: &std::path::Path,
    log_path: &std::path::Path,
    state: &backup_core::state::StoredState,
    recent_activity: &[String],
) -> String {
    let mut primary = String::new();
    primary.push_str("# Backup Sync Diagnostics\n");
    primary.push_str(&format!("Generated: {}\n\n", Utc::now()));
    primary.push_str("## Paths\n");
    primary.push_str(&format!(
        "Config file: {}\nState file: {}\nLog file: {}\n",
        redact_path(config_path),
        redact_path(state_path),
        redact_path(log_path)
    ));
    primary.push_str("\n## State summary\n");
    primary.push_str(&format!(
        "Last error: {}\nLast verify: {:?} (issues: {:?})\nRecent activity: {} items\n",
        redact_text(&format!("{:?}", state.last_error)),
        state.last_verify_ts,
        state.last_verify_issues,
        state.recent_activity.len()
    ));
    primary.push_str("\n## Recent activity (latest 20)\n");
    primary.push_str(&recent_activity.join("\n"));
    primary
}

/// Keep zip writing logic centralized.
fn write_zip_entry<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    name: &str,
    data: &str,
    opts: FileOptions,
) -> Result<(), ErrorEnvelope> {
    zip.start_file(name, opts).map_err(|e| {
        ErrorEnvelope::new(
            "DIAG_WRITE",
            format!("support::write_zip_entry failed to start {}: {}", name, e),
        )
    })?;
    zip.write_all(data.as_bytes()).map_err(|e| {
        ErrorEnvelope::new(
            "DIAG_WRITE",
            format!("support::write_zip_entry failed to write {}: {}", name, e),
        )
    })
}

#[tauri::command]
/// Provide a comprehensive bundle for support.
pub async fn export_diagnostic_bundle_cmd(
    correlation_id: Option<String>,
) -> Result<String, ErrorEnvelope> {
    let cid = correlation::cid("diag", correlation_id);
    info!(
        cid = %cid,
        action = "diagnostic_bundle_start",
        "gui diagnostic bundle requested"
    );
    let cfg = load_validated_config().map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_LOAD",
            format!(
                "support::export_diagnostic_bundle_cmd failed to load config: {}",
                e
            ),
        )
    })?;
    let config_path = paths::config_file_path().map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_PATH",
            format!(
                "support::export_diagnostic_bundle_cmd failed to resolve config path: {}",
                e
            ),
        )
    })?;
    let state_path = paths::state_file_path().map_err(|e| {
        ErrorEnvelope::new(
            "STATE_PATH",
            format!(
                "support::export_diagnostic_bundle_cmd failed to resolve state path: {}",
                e
            ),
        )
    })?;
    let log_path = backup_core::platform::paths::log_file_path().map_err(|e| {
        ErrorEnvelope::new(
            "LOG_PATH",
            format!(
                "support::export_diagnostic_bundle_cmd failed to resolve log path: {}",
                e
            ),
        )
    })?;
    let (state, _) = StateStore::load_or_default(state_path.clone()).map_err(|e| {
        ErrorEnvelope::new(
            "STATE_LOAD",
            format!(
                "support::export_diagnostic_bundle_cmd failed to load state: {}",
                e
            ),
        )
    })?;
    let status_result = get_status().await;

    let redacted = build_redacted(&cfg);
    let diff = build_diff(&cfg);
    let tail = tail_logs(&log_path, cfg.runtime.log_tail_lines);
    let recent_activity = recent_activity_lines(&state);
    let status_json = match status_result {
        Ok(status) => redact_text(&serialize_json(
            "support::export_diagnostic_bundle_cmd status",
            &status,
        )),
        Err(e) => format!(
            "support::export_diagnostic_bundle_cmd status unavailable: {}",
            e.message
        ),
    };

    let dest_dir = desktop_dir().ok_or_else(|| {
        ErrorEnvelope::new(
            "NO_DESKTOP",
            "support::export_diagnostic_bundle_cmd no desktop directory available".to_string(),
        )
    })?;
    let ts = Utc::now().format("%Y%m%d-%H%M%S");
    let stem = format!("BackupSync-support-{ts}");
    let (dest, file) = run_blocking_io(
        "gui::support::export_diagnostic_bundle_cmd create bundle file",
        || {
            create_exclusive_file(&dest_dir, &stem, "zip")
                .context("failed to reserve a unique support bundle filename")
        },
    )
    .map_err(|e| {
        ErrorEnvelope::new(
            "DIAG_WRITE",
            format!(
                "Could not save the support bundle: {} Hint: confirm the Desktop exists and is writable, then retry.",
                redact_text(&e.to_string())
            ),
        )
    })?;
    let mut incomplete = IncompleteFileGuard::new(dest.clone());
    let mut zip = ZipWriter::new(file);
    let opts = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let primary = build_primary_text(
        &config_path,
        &state_path,
        &log_path,
        &state,
        &recent_activity,
    );
    write_zip_entry(&mut zip, "diagnostics.txt", &primary, opts)?;
    write_zip_entry(
        &mut zip,
        "config-redacted.json",
        &serialize_json(
            "support::export_diagnostic_bundle_cmd redacted config",
            &redacted,
        ),
        opts,
    )?;
    write_zip_entry(
        &mut zip,
        "config-diff.json",
        &serialize_json("support::export_diagnostic_bundle_cmd config diff", &diff),
        opts,
    )?;
    write_zip_entry(&mut zip, "status.json", &status_json, opts)?;
    write_zip_entry(&mut zip, "log-tail.txt", &tail, opts)?;

    maybe_add_state(&mut zip, &state_path, opts)?;
    zip.finish().map_err(|e| {
        ErrorEnvelope::new(
            "DIAG_WRITE",
            format!(
                "Could not finalize the support bundle: {} Hint: confirm sufficient Desktop space and retry.",
                redact_text(&e.to_string())
            ),
        )
    })?;
    incomplete.commit();
    info!(
        cid = %cid,
        action = "diagnostic_bundle_complete",
        bundle_path = %redact_path(&dest),
        "gui diagnostic bundle written"
    );
    Ok(dest.display().to_string())
}

/// Include state for support without failing when missing.
fn maybe_add_state<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    state_path: &std::path::Path,
    opts: FileOptions,
) -> Result<(), ErrorEnvelope> {
    if state_path.exists() {
        let data = run_blocking_io("gui::support::maybe_add_state read state file", || {
            std::fs::read_to_string(state_path).with_context(|| {
                format!("support::maybe_add_state failed reading {:?}", state_path)
            })
        })
        .map_err(|e| {
            ErrorEnvelope::new(
                "STATE_READ",
                format!(
                    "support::maybe_add_state failed to read state file {}: {}",
                    redact_path(state_path),
                    e
                ),
            )
        })?;
        write_zip_entry(zip, "state.json", &redact_text(&data), opts)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{create_exclusive_file, write_probe};
    use std::fs;
    use std::io::{Read, Write};
    use std::path::PathBuf;

    fn test_dir(label: &str) -> PathBuf {
        let base = std::env::temp_dir();
        for attempt in 0..32 {
            let candidate = base.join(format!(
                "backup-sync-diagnostics-{label}-{}-{attempt}",
                std::process::id()
            ));
            match fs::create_dir(&candidate) {
                Ok(()) => return candidate,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("failed to create test directory: {error}"),
            }
        }
        panic!("failed to allocate a unique test directory")
    }

    #[test]
    fn exclusive_outputs_do_not_overwrite_predictable_name() {
        let dir = test_dir("output");
        let predictable = dir.join("BackupSync-support-20260101-010101.zip");
        fs::write(&predictable, b"existing").expect("write collision fixture");

        let (created, mut file) =
            create_exclusive_file(&dir, "BackupSync-support-20260101-010101", "zip")
                .expect("create exclusive output");
        file.write_all(b"new").expect("write exclusive output");
        drop(file);

        assert_ne!(created, predictable);
        assert_eq!(fs::read(&predictable).expect("read fixture"), b"existing");
        fs::remove_dir_all(dir).expect("remove test directory");
    }

    #[test]
    fn write_probe_never_touches_legacy_probe_file() {
        let dir = test_dir("legacy-probe");
        let legacy = dir.join(".backup_sync_probe");
        fs::write(&legacy, b"keep-me").expect("write legacy probe fixture");

        write_probe(&dir).expect("probe destination");

        assert_eq!(fs::read(&legacy).expect("read fixture"), b"keep-me");
        assert_eq!(fs::read_dir(&dir).expect("list directory").count(), 1);
        fs::remove_dir_all(dir).expect("remove test directory");
    }

    #[cfg(unix)]
    #[test]
    fn write_probe_does_not_follow_legacy_probe_symlink() {
        use std::os::unix::fs::symlink;

        let dir = test_dir("probe-symlink");
        let victim = dir.join("victim.txt");
        fs::write(&victim, b"unchanged").expect("write victim");
        let legacy = dir.join(".backup_sync_probe");
        symlink(&victim, &legacy).expect("create legacy probe symlink");

        write_probe(&dir).expect("probe destination");

        let mut contents = String::new();
        fs::File::open(&victim)
            .expect("open victim")
            .read_to_string(&mut contents)
            .expect("read victim");
        assert_eq!(contents, "unchanged");
        assert!(fs::symlink_metadata(&legacy)
            .expect("stat legacy symlink")
            .file_type()
            .is_symlink());
        fs::remove_dir_all(dir).expect("remove test directory");
    }
}
