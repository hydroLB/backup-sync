use anyhow::{anyhow, bail, Context, Result};
#[cfg(target_os = "macos")]
use daemon::integration::launchd;
#[cfg(target_os = "linux")]
use daemon::integration::systemd;
#[cfg(target_os = "windows")]
use daemon::integration::windows_service;
use std::path::PathBuf;

/// Purpose: Installs or prints the background service definition for the platform.
///
/// Inputs: service options including user scope, output path, and enable flags.
/// Outputs: `Ok(())` after writing or printing the manifest.
/// Ties to: CLI service management for launchd, systemd, or schtasks.
/// Side effects: Writes service manifests and runs system service commands.
/// Why: make daemon start on login with a single CLI command.
pub fn install_service(
    user: bool,
    output: Option<PathBuf>,
    log_path: Option<PathBuf>,
    print: bool,
    enable: bool,
    dry_run: bool,
) -> Result<()> {
    #[cfg(not(target_os = "linux"))]
    let user = user; // keep signature consistent; unused on non-Linux
    #[cfg(not(target_os = "linux"))]
    let _ = user;
    let exec = std::env::current_exe()
        .context("cli::install_service failed to determine current executable path")?
        .canonicalize()
        .context("cli::install_service failed to canonicalize executable path")?;
    let default_log = backup_core::platform::paths::log_file_path().ok();
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
                let status = std::process::Command::new("launchctl")
                    .args(["load", dest_str])
                    .status()
                    .context("cli::install_service failed to run launchctl load")?;
                if !status.success() {
                    bail!("cli::install_service launchctl load failed with {}", status);
                }
                let status = std::process::Command::new("launchctl")
                    .args(["enable", "system/com.backup_sync.daemon"])
                    .status()
                    .context("cli::install_service failed to run launchctl enable")?;
                if !status.success() {
                    bail!(
                        "cli::install_service launchctl enable failed with {}",
                        status
                    );
                }
                let status = std::process::Command::new("launchctl")
                    .args(["start", "com.backup_sync.daemon"])
                    .status()
                    .context("cli::install_service failed to run launchctl start")?;
                if !status.success() {
                    bail!(
                        "cli::install_service launchctl start failed with {}",
                        status
                    );
                }
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
                    let status = std::process::Command::new("systemctl")
                        .args(["--user", "daemon-reload"])
                        .status()
                        .context(
                            "cli::install_service failed to run systemctl --user daemon-reload",
                        )?;
                    if !status.success() {
                        bail!("cli::install_service systemctl --user daemon-reload failed");
                    }
                    let status = std::process::Command::new("systemctl")
                        .args(["--user", "enable", "--now", "backup-sync.service"])
                        .status()
                        .context("cli::install_service failed to run systemctl --user enable")?;
                    if !status.success() {
                        bail!("cli::install_service systemctl --user enable failed");
                    }
                } else {
                    println!("Enable with: systemctl --user daemon-reload && systemctl --user enable --now backup-sync.service");
                }
            } else {
                println!("System systemd unit written to {:?}.", dest);
                if enable {
                    let status = std::process::Command::new("systemctl")
                        .args(["daemon-reload"])
                        .status()
                        .context("cli::install_service failed to run systemctl daemon-reload")?;
                    if !status.success() {
                        bail!("cli::install_service systemctl daemon-reload failed");
                    }
                    let status = std::process::Command::new("systemctl")
                        .args(["enable", "--now", "backup-sync.service"])
                        .status()
                        .context("cli::install_service failed to run systemctl enable")?;
                    if !status.success() {
                        bail!("cli::install_service systemctl enable failed");
                    }
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
                let status = std::process::Command::new("schtasks")
                    .args([
                        "/Create",
                        "/TN",
                        "BackupSync",
                        "/XML",
                        &dest.display().to_string(),
                        "/F",
                    ])
                    .status()
                    .context("cli::install_service failed to run schtasks /Create")?;
                if !status.success() {
                    bail!("cli::install_service schtasks /Create failed");
                }
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
