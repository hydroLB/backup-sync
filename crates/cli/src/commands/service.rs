use anyhow::{anyhow, bail, Context, Result};
#[cfg(target_os = "macos")]
use backup_core::service::launchd;
#[cfg(target_os = "linux")]
use backup_core::service::systemd;
#[cfg(target_os = "windows")]
use backup_core::service::windows_service;
use backup_core::{load_validated_config, logging::cid, RuntimeTuning};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};
use tracing::warn;

const DAEMON_EXEC_ENV: &str = "BACKUP_SYNC_DAEMON_EXEC";

/// The service must execute the daemon binary, not the CLI wrapper.
fn resolve_daemon_exec() -> Result<PathBuf> {
    if let Ok(v) = std::env::var(DAEMON_EXEC_ENV) {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            let p = PathBuf::from(trimmed).canonicalize().with_context(|| {
                format!(
                    "cli::resolve_daemon_exec failed to canonicalize {DAEMON_EXEC_ENV} override"
                )
            })?;
            if !p.is_file() {
                bail!(
                    "cli::resolve_daemon_exec {DAEMON_EXEC_ENV} override is not a file: {:?}",
                    p
                );
            }
            return Ok(p);
        }
    }

    let current = std::env::current_exe()
        .context("cli::resolve_daemon_exec failed to determine current executable path")?
        .canonicalize()
        .context("cli::resolve_daemon_exec failed to canonicalize current executable path")?;
    let parent = current.parent().ok_or_else(|| {
        anyhow!(
            "cli::resolve_daemon_exec current executable has no parent: {:?}",
            current
        )
    })?;

    #[cfg(windows)]
    let candidate = parent.join("daemon.exe");
    #[cfg(not(windows))]
    let candidate = parent.join("daemon");

    if candidate.exists() {
        let p = candidate
            .canonicalize()
            .context("cli::resolve_daemon_exec failed to canonicalize daemon candidate")?;
        if p.is_file() {
            return Ok(p);
        }
    }

    bail!(
        "cli::resolve_daemon_exec could not locate daemon binary. Set {DAEMON_EXEC_ENV} or build/install the daemon. Current exe: {:?}",
        current
    );
}

/// Service command timeout and retry behavior should come from central config.
fn resolve_runtime_tuning() -> RuntimeTuning {
    match load_validated_config() {
        Ok(cfg) => cfg.runtime,
        Err(error) => {
            let correlation_id = cid("cli-service");
            warn!(
                cid = %correlation_id,
                action = "runtime_tuning_fallback",
                error = %error,
                "cli::resolve_runtime_tuning failed to load config; using defaults"
            );
            RuntimeTuning::default()
        }
    }
}

/// Keep subprocess IO bounded and resilient without ad-hoc per-call behavior.
fn run_service_command(cmd: &str, args: &[&str], context: &str) -> Result<()> {
    let runtime = resolve_runtime_tuning();
    let timeout = Duration::from_secs(runtime.service_command_timeout_seconds.max(1));
    let poll_interval = Duration::from_millis(runtime.service_command_poll_interval_ms.max(1));
    let retry_delay = Duration::from_millis(runtime.service_command_retry_delay_ms.max(1));

    let mut last_error: Option<anyhow::Error> = None;
    for attempt in 1..=2 {
        match run_service_command_once(cmd, args, context, timeout, poll_interval) {
            Ok(output) if output.status.success() => return Ok(()),
            Ok(output) => {
                last_error = Some(anyhow!(
                    "cli::run_service_command {} exited non-zero (attempt={} status={} stderr={})",
                    context,
                    attempt,
                    output.status,
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
            Err(error) => {
                last_error = Some(error.context(format!(
                    "cli::run_service_command {} failed on attempt {}",
                    context, attempt
                )));
            }
        }
        if attempt == 1 {
            std::thread::sleep(retry_delay);
        }
    }
    Err(last_error.unwrap_or_else(|| {
        anyhow!(
            "cli::run_service_command {} failed with unknown error",
            context
        )
    }))
}

/// Provide explicit timeout handling for each subprocess attempt.
fn run_service_command_once(
    cmd: &str,
    args: &[&str],
    context: &str,
    timeout: Duration,
    poll_interval: Duration,
) -> Result<Output> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("cli::run_service_command_once failed to spawn {}", context))?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => {
                return child.wait_with_output().with_context(|| {
                    format!(
                        "cli::run_service_command_once failed waiting for {}",
                        context
                    )
                });
            }
            Ok(None) => {}
            Err(error) => {
                return Err(anyhow!(
                    "cli::run_service_command_once failed polling {}: {}",
                    context,
                    error
                ));
            }
        }
        if start.elapsed() > timeout {
            if let Err(error) = child.kill() {
                warn!(
                    error = %error,
                    "cli::run_service_command_once failed to kill timed-out process for {}",
                    context
                );
            }
            if let Err(error) = child.wait() {
                warn!(
                    error = %error,
                    "cli::run_service_command_once failed waiting after timeout for {}",
                    context
                );
            }
            bail!(
                "cli::run_service_command_once {} timed out after {} seconds",
                context,
                timeout.as_secs()
            );
        }
        std::thread::sleep(poll_interval);
    }
}

/// Make daemon start on login with a single CLI command.
pub fn install_service(
    user: bool,
    output: Option<PathBuf>,
    log_path: Option<PathBuf>,
    print: bool,
    enable: bool,
    dry_run: bool,
) -> Result<()> {
    #[cfg(not(target_os = "linux"))]
    if user {
        let correlation_id = cid("cli-service");
        warn!(
            cid = %correlation_id,
            action = "user_flag_ignored",
            "cli::install_service `--user` is only applied on Linux; flag ignored on this platform"
        );
    }
    let exec = resolve_daemon_exec()
        .context("cli::install_service failed to resolve daemon executable")?;
    let default_log = match backup_core::platform::paths::log_file_path() {
        Ok(path) => Some(path),
        Err(error) => {
            let correlation_id = cid("cli-service");
            warn!(
                cid = %correlation_id,
                action = "default_log_path_unavailable",
                error = %error,
                "cli::install_service failed to resolve default log path; continuing without explicit log path"
            );
            None
        }
    };
    let log_path = log_path.or(default_log);
    if dry_run && print {
        println!("--dry-run and --print set: printing only, no writes/enables");
    }
    #[cfg(target_os = "macos")]
    {
        let dest = output.unwrap_or(
            launchd::default_plist_path()
                .context("cli::install_service failed to resolve launchd plist path")?,
        );
        let target = if print || dry_run {
            PathBuf::new()
        } else {
            dest.clone()
        };
        let plist = launchd::write_plist(target.as_path(), &exec, log_path.as_deref())
            .context("cli::install_service failed to write launchd plist")?;
        if print {
            println!("{}", plist);
        } else if !dry_run {
            println!("launchd plist written to {:?}", dest);
            if enable {
                let dest_str = dest.to_str().ok_or_else(|| {
                    anyhow!(
                        "cli::install_service invalid plist path encoding: {:?}",
                        dest
                    )
                })?;
                run_service_command("launchctl", &["load", dest_str], "launchctl load")?;
                run_service_command(
                    "launchctl",
                    &["enable", "system/com.backup_sync.daemon"],
                    "launchctl enable",
                )?;
                run_service_command(
                    "launchctl",
                    &["start", "com.backup_sync.daemon"],
                    "launchctl start",
                )?;
            } else {
                println!("Load/start with: launchctl load {:?} && launchctl enable com.backup_sync.daemon && launchctl start com.backup_sync.daemon", dest);
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        let dest = output.unwrap_or(
            systemd::default_unit_path(user)
                .context("cli::install_service failed to resolve systemd unit path")?,
        );
        let target = if print || dry_run {
            PathBuf::new()
        } else {
            dest.clone()
        };
        let unit = systemd::write_unit(target.as_path(), &exec, user, log_path.as_deref())
            .context("cli::install_service failed to write systemd unit")?;
        if print {
            println!("{}", unit);
        } else if !dry_run {
            if user {
                println!("User systemd unit written to {:?}.", dest);
                if enable {
                    run_service_command(
                        "systemctl",
                        &["--user", "daemon-reload"],
                        "systemctl --user daemon-reload",
                    )?;
                    run_service_command(
                        "systemctl",
                        &["--user", "enable", "--now", "backup-sync.service"],
                        "systemctl --user enable --now backup-sync.service",
                    )?;
                } else {
                    println!("Enable with: systemctl --user daemon-reload && systemctl --user enable --now backup-sync.service");
                }
            } else {
                println!("System systemd unit written to {:?}.", dest);
                if enable {
                    run_service_command(
                        "systemctl",
                        &["daemon-reload"],
                        "systemctl daemon-reload",
                    )?;
                    run_service_command(
                        "systemctl",
                        &["enable", "--now", "backup-sync.service"],
                        "systemctl enable --now backup-sync.service",
                    )?;
                } else {
                    println!("Enable with: sudo systemctl daemon-reload && sudo systemctl enable --now backup-sync.service");
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        let dest = output.unwrap_or(
            windows_service::default_task_xml_path()
                .context("cli::install_service failed to resolve schtasks xml path")?,
        );
        let target = if print || dry_run {
            PathBuf::new()
        } else {
            dest.clone()
        };
        let xml = windows_service::write_schtasks_xml(target.as_path(), &exec)
            .context("cli::install_service failed to write schtasks xml")?;
        if print {
            println!("{}", xml);
        } else if !dry_run {
            println!("Scheduled task XML written to {:?}.", dest);
            if enable {
                let xml_path = dest.display().to_string();
                run_service_command(
                    "schtasks",
                    &["/Create", "/TN", "BackupSync", "/XML", &xml_path, "/F"],
                    "schtasks /Create",
                )?;
            } else {
                println!(
                    "Register with: schtasks /Create /TN \"BackupSync\" /XML \"{}\" /F",
                    dest.display()
                );
            }
        }
    }
    Ok(())
}
