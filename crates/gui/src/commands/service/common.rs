use crate::commands::error::ErrorEnvelope;
use backup_core::config::model::RuntimeTuning;
use backup_core::io::{run_with_policy, BlockingIoPolicy, CancellationFlag};
use backup_core::load_validated_config;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};
use tracing::warn;

const DAEMON_EXEC_ENV: &str = "BACKUP_SYNC_DAEMON_EXEC";

/// Summary: Loads runtime tuning for service command execution.
///
/// Inputs: none.
///
/// Outputs: runtime tuning from config or defaults.
///
/// Side effects: Reads config from disk and logs warnings on fallback.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: service command timeouts and retries.
///
/// Why this exists: keep service command behavior configurable without hardcoded constants.
fn resolve_runtime_tuning() -> RuntimeTuning {
    match load_validated_config() {
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

/// Summary: Loads blocking I/O policy for service file operations.
///
/// Inputs: none.
///
/// Outputs: I/O policy from config or bootstrap defaults.
///
/// Side effects: Reads config from disk and logs warnings on fallback.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: service manifest writes.
///
/// Why this exists: keep timeout and retry behavior for local file writes centralized and configurable.
fn resolve_io_policy() -> BlockingIoPolicy {
    match load_validated_config() {
        Ok(cfg) => BlockingIoPolicy::from_config(&cfg),
        Err(e) => {
            warn!(
                "service::resolve_io_policy failed to load config; using defaults: {}",
                e
            );
            BlockingIoPolicy::bootstrap_defaults()
        }
    }
}

/// Summary: Resolve the daemon executable path for service installation.
///
/// Inputs: the current executable location and optional override environment variable.
///
/// Outputs: the canonical daemon executable path.
///
/// Side effects: Reads environment variables and filesystem metadata.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `load_exec_and_log` and service manifest generation.
///
/// Why this exists: Services must execute the daemon binary, not the GUI or CLI wrapper.
fn resolve_daemon_exec() -> Result<PathBuf, ErrorEnvelope> {
    if let Ok(v) = std::env::var(DAEMON_EXEC_ENV) {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            let p = PathBuf::from(trimmed);
            let p = p.canonicalize().map_err(|e| {
                ErrorEnvelope::new(
                    "SERVICE_EXEC",
                    format!(
                        "service::resolve_daemon_exec failed to canonicalize {} override {:?}: {}",
                        DAEMON_EXEC_ENV, p, e
                    ),
                )
            })?;
            if !p.is_file() {
                return Err(ErrorEnvelope::new(
                    "SERVICE_EXEC",
                    format!(
                        "service::resolve_daemon_exec {} override is not a file: {:?}",
                        DAEMON_EXEC_ENV, p
                    ),
                ));
            }
            return Ok(p);
        }
    }

    let current = std::env::current_exe().map_err(|e| {
        ErrorEnvelope::new(
            "SERVICE_EXEC",
            format!(
                "service::resolve_daemon_exec failed to find current executable: {}",
                e
            ),
        )
    })?;
    let parent = current.parent().ok_or_else(|| {
        ErrorEnvelope::new(
            "SERVICE_EXEC",
            format!(
                "service::resolve_daemon_exec current executable has no parent directory: {:?}",
                current
            ),
        )
    })?;

    #[cfg(windows)]
    let candidate = parent.join("daemon.exe");
    #[cfg(not(windows))]
    let candidate = parent.join("daemon");

    if candidate.exists() {
        let p = candidate.canonicalize().map_err(|e| {
            ErrorEnvelope::new(
                "SERVICE_EXEC",
                format!(
                    "service::resolve_daemon_exec failed to canonicalize daemon candidate {:?}: {}",
                    candidate, e
                ),
            )
        })?;
        if p.is_file() {
            return Ok(p);
        }
    }

    Err(ErrorEnvelope::new(
        "SERVICE_EXEC",
        format!(
            "service::resolve_daemon_exec could not locate daemon binary. Set {} to an explicit path or build the daemon (for dev: `cargo build -p daemon`). Current exe: {:?}",
            DAEMON_EXEC_ENV, current
        ),
    ))
}

/// Summary: Loads the current executable path and optional log path.
///
/// Inputs: the current process environment and platform log path resolver.
///
/// Outputs: the resolved executable path and optional log path.
///
/// Side effects: Reads environment and filesystem metadata for executable resolution.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: service installation workflows.
///
/// Why this exists: ensure service manifests reference the correct binary.
pub fn load_exec_and_log() -> Result<(PathBuf, Option<PathBuf>), ErrorEnvelope> {
    let exec = resolve_daemon_exec()?;
    let log_path = match backup_core::platform::paths::log_file_path() {
        Ok(path) => Some(path),
        Err(error) => {
            warn!(
                "service::load_exec_and_log failed to resolve log path; continuing without explicit log path: {}",
                error
            );
            None
        }
    };
    Ok((exec, log_path))
}

/// Summary: Runs a command and returns its output on success.
///
/// Inputs: the command, arguments, error code, and context string.
///
/// Outputs: the command output on success.
///
/// Side effects: Spawns system commands, waits for completion, and may sleep while polling.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: service enable and restart operations.
///
/// Why this exists: centralize command execution and error mapping.
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
                    if let Err(error) = child.kill() {
                        warn!(
                            "service::run_command failed to kill timed-out child process for {}: {}",
                            context, error
                        );
                    }
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

/// Summary: Runs a command with a single retry on failure.
///
/// Inputs: the command, arguments, error code, and context string.
///
/// Outputs: `Ok(())` when the command succeeds.
///
/// Side effects: Spawns commands and sleeps between retry attempts.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: service enable workflows that sometimes race system daemons.
///
/// Why this exists: improve reliability when system tools need a brief delay.
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

/// Summary: Writes text content to a destination file.
///
/// Inputs: the destination path, content, and error context string.
///
/// Outputs: `Ok(())` when the file is written.
///
/// Side effects: Writes manifest files to disk.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: service manifest generation.
///
/// Why this exists: keep manifest writing consistent across platforms.
pub fn write_text_file(dest: &Path, content: &str, context: &str) -> Result<(), ErrorEnvelope> {
    let io_policy = resolve_io_policy();
    run_with_policy(
        "service::write_text_file write manifest",
        &io_policy,
        CancellationFlag::none(),
        || {
            std::fs::write(dest, content).map_err(|e| {
                anyhow::anyhow!(e).context(format!(
                    "service::write_text_file {} failed to write {:?}",
                    context, dest
                ))
            })
        },
    )
    .map_err(|e| {
        ErrorEnvelope::new(
            "SERVICE_WRITE",
            format!("service::write_text_file {} failed: {}", context, e),
        )
    })
}

/// Summary: Service status payload returned to the GUI.
///
/// Inputs: derived from system checks and daemon IPC reachability.
///
/// Outputs: a serializable status structure.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: service status queries in the UI.
///
/// Why this exists: provide a consistent status schema for the frontend.
#[derive(Serialize)]
pub struct ServiceStatus {
    pub installed: bool,
    pub reachable: bool,
    pub message: String,
    pub fix_command: Option<String>,
    pub uptime_secs: Option<i64>,
    pub last_ipc_ts: Option<i64>,
}
