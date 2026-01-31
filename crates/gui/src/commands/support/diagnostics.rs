use crate::commands::status::get_status;
use crate::commands::{auth::SessionAuth, error::ErrorEnvelope, security};
use backup_core::config::load::default_config;
use backup_core::{load_config, platform::paths, state::store::StateStore};
use chrono::Utc;
use dirs::desktop_dir;
use fs2::free_space;
use serde::Serialize;
use serde_json::json;
use std::fs;
use std::io::{Seek, Write};
use tracing::warn;
use zip::write::FileOptions;
use zip::ZipWriter;

#[tauri::command]
/// Purpose: Generates a doctor report and writes it to the Desktop.
///
/// Inputs: an optional correlation id and auth state.
/// Outputs: the report path string or an error envelope.
/// Ties to: GUI diagnostics commands.
/// Side effects: Reads config/state data, writes a report file, and emits logs.
/// Why: provide a quick diagnostic snapshot for users.
pub async fn doctor_report_cmd(
    correlation_id: Option<String>,
    auth_state: tauri::State<'_, SessionAuth>,
) -> Result<String, ErrorEnvelope> {
    let cid = correlation_id.unwrap_or_else(|| format!("doctor-{}", Utc::now().timestamp_millis()));
    security::ensure_unlocked(&auth_state, Some(cid.clone()))?;
    eprintln!("[cid={}] doctor report start", cid);
    let cfg = load_config().map_err(|e| {
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
    let dest = dest_dir.join(format!("BackupSync-doctor-{}.txt", ts));
    fs::write(&dest, report).map_err(|e| {
        ErrorEnvelope::new(
            "DOCTOR_WRITE",
            format!("support::doctor_report_cmd failed to write report: {}", e),
        )
    })?;
    eprintln!("[cid={}] doctor report written -> {}", cid, dest.display());
    Ok(dest.display().to_string())
}

/// Purpose: Builds the doctor report body from config and state data.
///
/// Inputs: config, state, and resolved config and state paths.
/// Outputs: the report string content.
/// Ties to: doctor report generation.
/// Side effects: Reads filesystem metadata and writes a probe file via helpers.
/// Why: centralize report formatting.
fn build_doctor_report(
    cfg: &backup_core::Config,
    state: &backup_core::state::StoredState,
    config_path: &std::path::Path,
    state_path: &std::path::Path,
) -> Result<String, ErrorEnvelope> {
    let mut report = String::new();
    report.push_str("# Backup Sync Doctor Report\n");
    report.push_str(&format!("Generated: {}\n\n", Utc::now()));
    report.push_str(&format!("Config file: {:?}\n", config_path));
    report.push_str(&format!("State file: {:?}\n", state_path));
    let dest_path = cfg
        .destinations
        .get(0)
        .map(|d| d.path.clone())
        .unwrap_or_else(|| cfg.backup_root.clone());
    report.push_str(&format!("Backup destination: {:?}\n", dest_path));
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

/// Purpose: Appends missing watched path information to the report.
///
/// Inputs: the config and the report buffer.
/// Outputs: `()` after appending text.
/// Ties to: doctor report generation.
/// Side effects: Reads filesystem metadata to detect missing paths.
/// Why: surface missing paths in diagnostics.
fn append_missing_paths(cfg: &backup_core::Config, out: &mut String) {
    if cfg.watched.is_empty() {
        out.push_str("Problem: no watched paths configured.\n");
    }
    let (_existing, missing): (Vec<_>, Vec<_>) = cfg.watched.iter().partition(|w| w.path.exists());
    if !missing.is_empty() {
        out.push_str(&format!("Missing watched paths ({}):\n", missing.len()));
        for m in missing {
            out.push_str(&format!("  {:?}\n", m.path));
        }
    }
}

/// Purpose: Appends free space information to the report.
///
/// Inputs: the config, destination path, and report buffer.
/// Outputs: `()` after appending text.
/// Ties to: doctor report generation.
/// Side effects: Reads filesystem free space metadata.
/// Why: surface free space issues in diagnostics.
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

/// Purpose: Appends a write probe result to the report.
///
/// Inputs: the destination path and report buffer.
/// Outputs: `()` after appending text.
/// Ties to: doctor report generation.
/// Side effects: Writes and removes a probe file to verify writability.
/// Why: surface writability issues in diagnostics.
fn append_write_probe(_cfg: &backup_core::Config, dest_path: &std::path::Path, out: &mut String) {
    let probe = dest_path.join(".backup_sync_probe");
    match fs::write(&probe, b"probe") {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            out.push_str("Write check: OK\n");
        }
        Err(e) => out.push_str(&format!(
            "Problem: cannot write to backup destination: {}\n",
            e
        )),
    }
}

#[derive(Serialize)]
/// Purpose: Redacted configuration payload for diagnostics bundles.
///
/// Inputs: derived from the config.
/// Outputs: a redacted config snapshot.
/// Ties to: diagnostic bundle generation.
/// Side effects: None.
/// Why: include config metadata without sensitive path details.
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
    auth_unlock_seconds: u64,
    tray_tooltip_refresh_seconds: u64,
    log_tail_lines: usize,
    simulation_sample_limit: usize,
    scan_capacity_multiplier: usize,
}

/// Purpose: Builds a redacted config payload for diagnostics.
///
/// Inputs: the config.
/// Outputs: a redacted config struct.
/// Ties to: diagnostic bundle generation.
/// Side effects: None.
/// Why: share configuration metadata safely.
fn build_redacted(cfg: &backup_core::Config) -> RedactedConfig {
    RedactedConfig {
        backup_root: cfg.backup_root.display().to_string(),
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
        auth_unlock_seconds: cfg.runtime.auth_unlock_seconds,
        tray_tooltip_refresh_seconds: cfg.runtime.tray_tooltip_refresh_seconds,
        log_tail_lines: cfg.runtime.log_tail_lines,
        simulation_sample_limit: cfg.runtime.simulation_sample_limit,
        scan_capacity_multiplier: cfg.planning.scan_capacity_multiplier,
    }
}

/// Purpose: Builds a diff between current config and defaults for diagnostics.
///
/// Inputs: the config.
/// Outputs: a JSON diff payload.
/// Ties to: diagnostic bundle generation.
/// Side effects: Reads platform defaults for comparison.
/// Why: surface deviations from defaults in support bundles.
fn build_diff(cfg: &backup_core::Config) -> serde_json::Value {
    let defaults = match default_config() {
        Ok(defaults) => defaults,
        Err(e) => {
            warn!(
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
        "auth_unlock_seconds": (cfg.runtime.auth_unlock_seconds, defaults.runtime.auth_unlock_seconds),
        "tray_tooltip_refresh_seconds": (cfg.runtime.tray_tooltip_refresh_seconds, defaults.runtime.tray_tooltip_refresh_seconds),
        "log_tail_lines": (cfg.runtime.log_tail_lines, defaults.runtime.log_tail_lines),
        "simulation_sample_limit": (cfg.runtime.simulation_sample_limit, defaults.runtime.simulation_sample_limit),
        "scan_capacity_multiplier": (cfg.planning.scan_capacity_multiplier, defaults.planning.scan_capacity_multiplier),
    })
}

/// Purpose: Returns the tail of the log file for diagnostics.
///
/// Inputs: the log file path.
/// Outputs: the tail string.
/// Ties to: diagnostic bundle generation.
/// Side effects: Reads the log file from disk.
/// Why: include recent logs in support bundles.
fn tail_logs(path: &std::path::Path, max_lines: usize) -> String {
    let logs = match std::fs::read_to_string(path) {
        Ok(data) => data,
        Err(e) => {
            return format!(
                "support::tail_logs failed to read log file {:?}: {}",
                path, e
            );
        }
    };
    logs.lines()
        .rev()
        .take(max_lines)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n")
}

/// Purpose: Serializes a value to pretty JSON with a labeled error message on failure.
///
/// Inputs: a label string and a serializable value reference.
/// Outputs: a JSON string or an error message string.
/// Ties to: diagnostic bundle generation.
/// Side effects: None.
/// Why: avoid silent serialization failures while keeping the bundle readable.
fn serialize_json<T: Serialize>(label: &str, value: &T) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(data) => data,
        Err(e) => format!("{} failed to serialize JSON: {}", label, e),
    }
}

/// Purpose: Formats recent activity entries for diagnostics.
///
/// Inputs: stored state.
/// Outputs: a vector of formatted activity strings.
/// Ties to: diagnostic bundle generation.
/// Side effects: None.
/// Why: include recent activity in support bundles.
fn recent_activity_lines(state: &backup_core::state::StoredState) -> Vec<String> {
    state
        .recent_activity
        .iter()
        .rev()
        .take(20)
        .map(|a| format!("{} • {} bytes • {}", a.path, a.bytes, a.ts))
        .collect()
}

/// Purpose: Builds the primary diagnostic text file content.
///
/// Inputs: paths, state, and recent activity lines.
/// Outputs: the diagnostic text content.
/// Ties to: diagnostic bundle generation.
/// Side effects: None.
/// Why: provide a human readable summary in the bundle.
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
        "Config file: {:?}\nState file: {:?}\nLog file: {:?}\n",
        config_path, state_path, log_path
    ));
    primary.push_str("\n## State summary\n");
    primary.push_str(&format!(
        "Last error: {:?}\nLast verify: {:?} (issues: {:?})\nRecent activity: {} items\n",
        state.last_error,
        state.last_verify_ts,
        state.last_verify_issues,
        state.recent_activity.len()
    ));
    primary.push_str("\n## Recent activity (latest 20)\n");
    primary.push_str(&recent_activity.join("\n"));
    primary
}

/// Purpose: Writes a text entry to the diagnostics zip bundle.
///
/// Inputs: the zip writer, entry name, data, and options.
/// Outputs: `Ok(())` when the entry is written.
/// Ties to: diagnostic bundle generation.
/// Side effects: Writes entries into the diagnostics zip archive.
/// Why: keep zip writing logic centralized.
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
/// Purpose: Exports a diagnostics bundle zip to the Desktop.
///
/// Inputs: an optional correlation id and auth state.
/// Outputs: the path to the bundle or an error envelope.
/// Ties to: GUI diagnostics actions.
/// Side effects: Reads config/state/logs and writes a diagnostics zip bundle.
/// Why: provide a comprehensive bundle for support.
pub async fn export_diagnostic_bundle_cmd(
    correlation_id: Option<String>,
    auth_state: tauri::State<'_, SessionAuth>,
) -> Result<String, ErrorEnvelope> {
    let cid = correlation_id.unwrap_or_else(|| format!("diag-{}", Utc::now().timestamp_millis()));
    security::ensure_unlocked(&auth_state, Some(cid.clone()))?;
    eprintln!("[cid={}] diagnostic bundle start", cid);
    let cfg = load_config().map_err(|e| {
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
        Ok(status) => serialize_json("support::export_diagnostic_bundle_cmd status", &status),
        Err(e) => format!(
            "support::export_diagnostic_bundle_cmd status unavailable: {}",
            e.message
        ),
    };

    let dest_dir = desktop_dir().ok_or_else(|| {
        ErrorEnvelope::new(
            "NO_DESKTOP",
            format!("support::export_diagnostic_bundle_cmd no desktop directory available"),
        )
    })?;
    let ts = Utc::now().format("%Y%m%d-%H%M%S");
    let dest = dest_dir.join(format!("BackupSync-support-{}.zip", ts));
    let file = std::fs::File::create(&dest).map_err(|e| {
        ErrorEnvelope::new(
            "DIAG_WRITE",
            format!(
                "support::export_diagnostic_bundle_cmd failed to create bundle: {}",
                e
            ),
        )
    })?;
    let mut zip = ZipWriter::new(file);
    let opts = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let primary = build_primary_text(
        &config_path,
        &state_path,
        &log_path,
        &state,
        &recent_activity,
    );
    write_zip_entry(&mut zip, "diagnostics.txt", &primary, opts.clone())?;
    write_zip_entry(
        &mut zip,
        "config-redacted.json",
        &serialize_json(
            "support::export_diagnostic_bundle_cmd redacted config",
            &redacted,
        ),
        opts.clone(),
    )?;
    write_zip_entry(
        &mut zip,
        "config-diff.json",
        &serialize_json("support::export_diagnostic_bundle_cmd config diff", &diff),
        opts.clone(),
    )?;
    write_zip_entry(&mut zip, "status.json", &status_json, opts.clone())?;
    write_zip_entry(&mut zip, "log-tail.txt", &tail, opts.clone())?;

    maybe_add_state(&mut zip, &state_path, opts)?;
    zip.finish().map_err(|e| {
        ErrorEnvelope::new(
            "DIAG_WRITE",
            format!(
                "support::export_diagnostic_bundle_cmd failed to finalize bundle: {}",
                e
            ),
        )
    })?;
    eprintln!(
        "[cid={}] diagnostic bundle written -> {}",
        cid,
        dest.display()
    );
    Ok(dest.display().to_string())
}

/// Purpose: Adds state.json to the diagnostics bundle when it exists.
///
/// Inputs: the zip writer, state path, and zip options.
/// Outputs: `Ok(())` after writing the entry when present.
/// Ties to: diagnostic bundle generation.
/// Side effects: Reads the state file and writes it into the zip archive.
/// Why: include state for support without failing when missing.
fn maybe_add_state<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    state_path: &std::path::Path,
    opts: FileOptions,
) -> Result<(), ErrorEnvelope> {
    if state_path.exists() {
        let data = std::fs::read_to_string(state_path).map_err(|e| {
            ErrorEnvelope::new(
                "STATE_READ",
                format!(
                    "support::maybe_add_state failed to read state file {:?}: {}",
                    state_path, e
                ),
            )
        })?;
        write_zip_entry(zip, "state.json", &data, opts)?;
    }
    Ok(())
}
