use crate::commands::error::ErrorEnvelope;
use backup_core::config::model::RuntimeTuning;
use backup_core::load_config;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};
use tracing::warn;

/// Purpose: Loads runtime tuning for service command execution.
///
/// Inputs: none.
/// Outputs: runtime tuning from config or defaults.
/// Ties to: service command timeouts and retries.
/// Side effects: Reads config from disk and logs warnings on fallback.
/// Why: keep service command behavior configurable without hardcoded constants.
fn resolve_runtime_tuning() -> RuntimeTuning {
    match load_config() {
        Ok(cfg) => cfg.runtime,
        Err(e) => {
            warn!(
                "service::resolve_runtime_tuning failed to load config; using defaults: {}",
                e
            );
            RuntimeTuning::default()
        }
    }
}

/// Purpose: Loads the current executable path and optional log path.
///
/// Inputs: the current process environment and platform log path resolver.
/// Outputs: the resolved executable path and optional log path.
/// Ties to: service installation workflows.
/// Side effects: Reads environment and filesystem metadata for executable resolution.
/// Why: ensure service manifests reference the correct binary.
pub fn load_exec_and_log() -> Result<(PathBuf, Option<PathBuf>), ErrorEnvelope> {
    let exec = std::env::current_exe().map_err(|e| {
        ErrorEnvelope::new(
            "SERVICE_EXEC",
            format!(
                "service::load_exec_and_log failed to find executable: {}",
                e
            ),
        )
    })?;
    let exec = exec.canonicalize().map_err(|e| {
        ErrorEnvelope::new(
            "SERVICE_EXEC",
            format!(
                "service::load_exec_and_log failed to canonicalize executable: {}",
                e
            ),
        )
    })?;
    let log_path = backup_core::platform::paths::log_file_path().ok();
    Ok((exec, log_path.map(PathBuf::from)))
}

/// Purpose: Runs a command and returns its output on success.
///
/// Inputs: the command, arguments, error code, and context string.
/// Outputs: the command output on success.
/// Ties to: service enable and restart operations.
/// Side effects: Spawns system commands, waits for completion, and may sleep while polling.
/// Why: centralize command execution and error mapping.
pub fn run_command(
    cmd: &str,
    args: &[&str],
    code: &'static str,
    context: &str,
) -> Result<Output, ErrorEnvelope> {
    let runtime = resolve_runtime_tuning();
    let timeout = Duration::from_secs(runtime.service_command_timeout_seconds);
    let poll_interval = Duration::from_millis(runtime.service_command_poll_interval_ms);
    let mut child = Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            ErrorEnvelope::new(
                code,
                format!("service::run_command failed to run {}: {}", context, e),
            )
        })?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => {
                let out = child.wait_with_output().map_err(|e| {
                    ErrorEnvelope::new(
                        code,
                        format!("service::run_command failed to wait {}: {}", context, e),
                    )
                })?;
                if !out.status.success() {
                    return Err(ErrorEnvelope::new(
                        code,
                        format!(
                            "service::run_command {} failed: {}",
                            context,
                            String::from_utf8_lossy(&out.stderr)
                        ),
                    ));
                }
                return Ok(out);
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return Err(ErrorEnvelope::new(
                        code,
                        format!(
                            "service::run_command {} timed out after {} seconds",
                            context,
                            timeout.as_secs()
                        ),
                    ));
                }
                std::thread::sleep(poll_interval);
            }
            Err(e) => {
                return Err(ErrorEnvelope::new(
                    code,
                    format!("service::run_command failed to poll {}: {}", context, e),
                ));
            }
        }
    }
}

/// Purpose: Runs a command with a single retry on failure.
///
/// Inputs: the command, arguments, error code, and context string.
/// Outputs: `Ok(())` when the command succeeds.
/// Ties to: service enable workflows that sometimes race system daemons.
/// Side effects: Spawns commands and sleeps between retry attempts.
/// Why: improve reliability when system tools need a brief delay.
pub fn run_command_with_retry(
    cmd: &str,
    args: &[&str],
    code: &'static str,
    context: &str,
) -> Result<(), ErrorEnvelope> {
    let runtime = resolve_runtime_tuning();
    let delay = Duration::from_millis(runtime.service_command_retry_delay_ms);
    let attempt = || run_command(cmd, args, code, context).map(|_| ());
    attempt().or_else(|e| {
        std::thread::sleep(delay);
        attempt().map_err(|_| e)
    })
}

/// Purpose: Writes text content to a destination file.
///
/// Inputs: the destination path, content, and error context string.
/// Outputs: `Ok(())` when the file is written.
/// Ties to: service manifest generation.
/// Side effects: Writes manifest files to disk.
/// Why: keep manifest writing consistent across platforms.
pub fn write_text_file(dest: &Path, content: &str, context: &str) -> Result<(), ErrorEnvelope> {
    std::fs::write(dest, content).map_err(|e| {
        ErrorEnvelope::new(
            "SERVICE_WRITE",
            format!("service::write_text_file {} failed: {}", context, e),
        )
    })
}

/// Purpose: Service status payload returned to the GUI.
///
/// Inputs: derived from system checks and daemon IPC reachability.
/// Outputs: a serializable status structure.
/// Ties to: service status queries in the UI.
/// Side effects: None.
/// Why: provide a consistent status schema for the frontend.
#[derive(Serialize)]
pub struct ServiceStatus {
    pub installed: bool,
    pub reachable: bool,
    pub message: String,
    pub fix_command: Option<String>,
    pub uptime_secs: Option<i64>,
    pub last_ipc_ts: Option<i64>,
}
