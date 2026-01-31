use crate::prompt::{expand_tilde, prompt_path, prompt_string, prompt_yes};
use anyhow::{Context, Result};
use backup_core::{
    config::model::{Destination, WatchedKind, WatchedPath},
    load_config,
    platform::paths,
    save_config, validate,
};
use std::path::PathBuf;

/// Purpose: Runs an interactive setup wizard for initial configuration.
///
/// Inputs: none, relies on user prompts.
/// Outputs: `Ok(())` after saving config and optionally running a backup.
/// Ties to: CLI initialization flow and config persistence.
/// Side effects: Prompts the user and writes the config file to disk.
/// Why: guide first time setup for backup configuration.
pub async fn init_wizard() -> Result<()> {
    println!("Backup Sync setup - just answer a couple quick questions.");
    let mut cfg = load_config().context("cli::init_wizard failed to load config for setup")?;
    if cfg.destinations.is_empty() {
        cfg.destinations.push(Destination {
            id: "default".into(),
            path: cfg.backup_root.clone(),
            label: Some("Primary".into()),
            max_backups_per_file: Some(cfg.max_backups_per_file),
        });
    }

    // Backup destination
    let current_root = cfg.backup_root.display().to_string();
    let backup_root = prompt_path(
        &format!(
            "Where should backups be stored? [default: {}]",
            current_root
        ),
        Some(&current_root),
    )
    .context("cli::init_wizard failed to prompt for backup root")?;
    cfg.backup_root = backup_root;
    if let Some(dest) = cfg.destinations.get_mut(0) {
        dest.path = cfg.backup_root.clone();
    }

    // Watched paths
    if cfg.watched.is_empty() {
        println!("No folders are being backed up yet. Let's add at least one.");
    } else {
        println!("Currently watched:");
        for w in &cfg.watched {
            println!("  - {:?} ({:?})", w.path, w.kind);
        }
    }

    while cfg.watched.is_empty()
        || prompt_yes(
            "Add another file/folder to protect?",
            cfg.watched.is_empty(),
        )
        .context("cli::init_wizard failed to prompt for additional watched path")?
    {
        let path = prompt_string("Enter the full path to back up (or leave blank to stop):")
            .context("cli::init_wizard failed to prompt for watched path")?;
        if path.is_empty() {
            if cfg.watched.is_empty() {
                println!("You need at least one path to continue.");
                continue;
            } else {
                break;
            }
        }
        let kind_dir = prompt_yes("Is this a folder? (y = folder, n = single file)", true)
            .context("cli::init_wizard failed to prompt for watched kind")?;
        let resolved = PathBuf::from(expand_tilde(&path));
        if !resolved.exists() {
            println!("That path does not exist. Please choose an existing file or folder.");
            continue;
        }
        let watched = WatchedPath {
            path: resolved,
            kind: if kind_dir {
                WatchedKind::Directory
            } else {
                WatchedKind::File
            },
            enabled: true,
            destination_id: "default".into(),
            max_backups_per_file: None,
        };
        println!("Added {:?}", watched.path);
        cfg.watched.push(watched);
    }

    validate(&cfg).context("cli::init_wizard config invalid after setup")?;
    save_config(&cfg).context("cli::init_wizard failed to save config after setup")?;
    println!(
        "Config saved to {:?}",
        paths::config_file_path().context("cli::init_wizard failed to resolve config path")?
    );

    if prompt_yes("Run a backup right now?", true)
        .context("cli::init_wizard failed to prompt for run now")?
    {
        super::run::run_once(false)
            .await
            .context("cli::init_wizard run-once failed")?;
    } else {
        println!("Skip for now. You can run one anytime with: backup-sync run-once");
    }
    Ok(())
}
