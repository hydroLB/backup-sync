use super::defaults::default_config;
use crate::config::model::Config;
use crate::platform::paths::config_file_path;
use anyhow::{Context, Result};
use std::{fs, path::Path};

/// Purpose: Loads the user config from the default path or creates one with defaults.
///
/// Inputs: none.
/// Outputs: a parsed `Config` ready for validation and use.
/// Ties to: CLI, daemon, and GUI startup workflows.
/// Side effects: Reads the config file and may write a default config to disk.
/// Why: provide a single source of truth for config loading without implicit mutations.
pub fn load_config() -> Result<Config> {
    let path = config_file_path()?;
    if path.exists() {
        let raw = fs::read_to_string(&path).with_context(|| {
            format!(
                "config::load_config failed reading config file at {:?}",
                path
            )
        })?;
        let cfg: Config = toml::from_str(&raw).with_context(|| {
            format!(
                "config::load_config failed parsing config TOML at {:?}",
                path
            )
        })?;
        Ok(cfg)
    } else {
        let cfg = default_config()?;
        save_config(&cfg)?;
        Ok(cfg)
    }
}

/// Purpose: Saves the config to the default path on disk.
///
/// Inputs: a reference to the config to persist.
/// Outputs: `Ok(())` when the file is written.
/// Ties to: config initialization and UI driven updates.
/// Side effects: Creates config directories and writes the config file to disk.
/// Why: ensure a single, consistent location for config persistence.
pub fn save_config(cfg: &Config) -> Result<()> {
    let path = config_file_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "config::save_config failed creating config directory {:?}",
                parent
            )
        })?;
    }
    let raw = toml::to_string_pretty(cfg)
        .context("config::save_config failed to serialize config to TOML")?;
    fs::write(&path, raw).with_context(|| {
        format!(
            "config::save_config failed writing config file at {:?}",
            path
        )
    })?;
    Ok(())
}

/// Purpose: Loads a config from an explicit path.
///
/// Inputs: the filesystem path to the config file.
/// Outputs: a parsed `Config` value.
/// Ties to: diagnostics and tests that load configs from fixtures.
/// Side effects: Reads the config file from disk.
/// Why: allow callers to load configs from non-standard locations.
pub fn load_from_path(path: &Path) -> Result<Config> {
    let raw = std::fs::read_to_string(path).with_context(|| {
        format!(
            "config::load_from_path failed reading config file at {:?}",
            path
        )
    })?;
    let cfg: Config = toml::from_str(&raw).with_context(|| {
        format!(
            "config::load_from_path failed parsing config TOML at {:?}",
            path
        )
    })?;
    Ok(cfg)
}
