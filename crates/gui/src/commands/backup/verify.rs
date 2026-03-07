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
use tracing::info;

#[derive(Serialize)]
/// Summary: Payload describing verification results.
///
/// Inputs: derived from verify runs.
///
/// Outputs: a serializable summary.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI verification results.
///
/// Why this exists: show verification status in the UI.
pub struct VerifyResult {
    pub ok: usize,
    pub bad: usize,
    pub last_verify_ts: Option<i64>,
    pub last_verify_status: Option<String>,
    pub last_verify_issues: Option<usize>,
}

#[tauri::command]
/// Summary: Runs a verification pass over recent backups.
///
/// Inputs: an optional correlation id.
///
/// Outputs: a `VerifyResult` or an error envelope.
///
/// Side effects: Reads config/state, hashes backup files, and writes updated state.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI verify actions.
///
/// Why this exists: allow users to verify backup integrity on demand.
pub async fn verify_cmd(correlation_id: Option<String>) -> Result<VerifyResult, ErrorEnvelope> {
    let cid = correlation::cid("verify", correlation_id);
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
/// Summary: Exports a health report to the Desktop directory.
///
/// Inputs: an optional correlation id.
///
/// Outputs: the path to the report file or an error envelope.
///
/// Side effects: Reads config/state and writes a health report file.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI diagnostics actions.
///
/// Why this exists: allow users to share health summaries.
pub async fn export_health_report_cmd(
    correlation_id: Option<String>,
) -> Result<String, ErrorEnvelope> {
    let cid = correlation::cid("health", correlation_id);
    info!(
        cid = %cid,
        action = "export_health_start",
        "gui health report export requested"
    );
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
    let status = fetch_status_snapshot().await;
    let report = build_health_report(&cfg, &state, &state_path, &status);
    let dest = write_health_report(&report).map_err(|e| {
        ErrorEnvelope::new(
            "HEALTH_WRITE",
            format!(
                "[cid={}] export_health_report_cmd failed to write report: {}",
                cid, e
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

/// Summary: Fetches a status snapshot string for the health report.
///
/// Inputs: none.
///
/// Outputs: a formatted status snapshot string.
///
/// Side effects: Performs an IPC status request.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: health report generation.
///
/// Why this exists: include current daemon status in diagnostics.
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

/// Summary: Builds the health report content from config, state, and status.
///
/// Inputs: config, state, state path, and status text.
///
/// Outputs: the full report string.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: health report export.
///
/// Why this exists: keep report formatting centralized.
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
    report.push_str("# Local Backup Manager Health Report\n");
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

/// Summary: Writes the health report to the Desktop directory.
///
/// Inputs: the report content.
///
/// Outputs: the written file path.
///
/// Side effects: Writes the report file to disk.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: health report export.
///
/// Why this exists: persist the report for sharing and diagnostics.
fn write_health_report(report: &str) -> Result<std::path::PathBuf, ErrorEnvelope> {
    let dest_dir = desktop_dir()
        .ok_or_else(|| ErrorEnvelope::new("NO_DESKTOP", "No desktop directory available"))?;
    let ts = Utc::now().format("%Y%m%d-%H%M%S");
    let dest = dest_dir.join(format!("BackupSync-health-{}.txt", ts));
    run_blocking_io(
        "gui::backup::verify::write_health_report write file",
        || {
            std::fs::write(&dest, report.as_bytes()).with_context(|| {
                format!(
                    "backup::verify::write_health_report failed writing report at {:?}",
                    dest
                )
            })
        },
    )
    .map_err(|e| {
        ErrorEnvelope::new(
            "HEALTH_WRITE",
            format!("backup::verify::write_health_report failed: {}", e),
        )
    })?;
    Ok(dest)
}
