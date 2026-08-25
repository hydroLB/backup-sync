use super::defaults::default_config;
use super::normalize::normalize_loaded_config;
use crate::config::model::Config;
use crate::config::validate::validate;
use crate::io::{run_with_policy, BlockingIoPolicy, CancellationFlag};
use crate::platform::paths::config_file_path;
use anyhow::{bail, Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

const ENV_CONFIG_PATH: &str = "BACKUP_SYNC_CONFIG";
const ENV_INTERVAL_SECONDS: &str = "BACKUP_SYNC_INTERVAL_SECONDS";
const ENV_SAFE_MODE: &str = "BACKUP_SYNC_SAFE_MODE";

enum ConfigSource {
    DefaultPath(PathBuf),
    ExplicitPath { path: PathBuf, reason: &'static str },
}

impl ConfigSource {
    /// Centralize source path rendering for consistent error messages.
    fn path(&self) -> &Path {
        match self {
            Self::DefaultPath(path) => path,
            Self::ExplicitPath { path, .. } => path,
        }
    }

    /// Explicit paths must fail if missing while default path may be bootstrapped.
    fn is_default_path(&self) -> bool {
        matches!(self, Self::DefaultPath(_))
    }

    /// Keep invalid-config diagnostics consistent across all entrypoints.
    fn description(&self) -> String {
        match self {
            Self::DefaultPath(path) => format!("default path {:?}", path),
            Self::ExplicitPath { path, reason } => {
                format!("explicit path {:?} ({reason})", path)
            }
        }
    }
}

/// Ensure every entrypoint resolves config location consistently.
fn resolve_config_source(path_override: Option<&Path>) -> Result<ConfigSource> {
    if let Some(path) = path_override {
        return Ok(ConfigSource::ExplicitPath {
            path: path.to_path_buf(),
            reason: "explicit argument",
        });
    }

    if let Some(raw) = std::env::var_os(ENV_CONFIG_PATH)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    {
        return Ok(ConfigSource::ExplicitPath {
            path: raw,
            reason: "BACKUP_SYNC_CONFIG env override",
        });
    }

    Ok(ConfigSource::DefaultPath(config_file_path()?))
}

/// Centralize parsing and error shape for config env overrides.
fn read_non_empty_env(name: &str) -> Result<Option<String>> {
    match std::env::var(name) {
        Ok(value) => {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed))
            }
        }
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => {
            bail!("config::env_layering invalid non-UTF-8 value in {name}")
        }
    }
}

/// Keep env parse diagnostics consistent across entrypoints.
fn parse_env_u64(name: &str, raw: &str) -> Result<u64> {
    raw.parse::<u64>().with_context(|| {
        format!("config::env_layering invalid unsigned integer for {name}: {raw:?}")
    })
}

/// Support operational toggles without ambiguous boolean parsing.
fn parse_env_bool(name: &str, raw: &str) -> Result<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => bail!(
            "config::env_layering invalid boolean for {name}: {raw:?} (accepted: true/false/1/0/yes/no/on/off)"
        ),
    }
}

/// Allow deterministic operational overrides without editing config files.
fn apply_env_layering(mut cfg: Config) -> Result<Config> {
    if let Some(raw) = read_non_empty_env(ENV_INTERVAL_SECONDS)? {
        cfg.interval_seconds = parse_env_u64(ENV_INTERVAL_SECONDS, &raw)?;
    }

    if let Some(raw) = read_non_empty_env(ENV_SAFE_MODE)? {
        cfg.safe_mode = parse_env_bool(ENV_SAFE_MODE, &raw)?;
    }

    Ok(cfg)
}

/// Unify file loading behavior before env layering and validation.
fn load_from_source(source: &ConfigSource) -> Result<Config> {
    let path = source.path();
    let io_policy = BlockingIoPolicy::bootstrap_defaults();

    if path.exists() {
        let raw = run_with_policy(
            "config::load_config read config",
            &io_policy,
            CancellationFlag::none(),
            || {
                fs::read_to_string(path).with_context(|| {
                    format!(
                        "config::load_config failed reading config file at {:?}",
                        path
                    )
                })
            },
        )?;
        let cfg: Config = toml::from_str(&raw).with_context(|| {
            format!(
                "config::load_config failed parsing config TOML at {:?}",
                path
            )
        })?;
        return Ok(normalize_loaded_config(cfg));
    }

    if source.is_default_path() {
        let cfg = default_config()?;
        save_config(&cfg)?;
        return Ok(cfg);
    }

    bail!(
        "config::load_config expected config file at explicit path {:?} but it does not exist",
        path
    )
}

/// Keep source-aware diagnostics consistent and avoid duplicated load logic.
fn load_with_source(path_override: Option<&Path>) -> Result<(Config, ConfigSource)> {
    let source = resolve_config_source(path_override)?;
    let cfg = load_from_source(&source)?;
    let cfg = apply_env_layering(cfg)?;
    Ok((cfg, source))
}

/// Provide a single source of truth for config loading behavior.
pub fn load_config() -> Result<Config> {
    let (cfg, _) = load_with_source(None)?;
    Ok(cfg)
}

/// Ensure all entrypoints fail fast on invalid config with the same error model.
pub fn load_validated_config() -> Result<Config> {
    let (cfg, source) = load_with_source(None)?;
    validate(&cfg).with_context(|| {
        format!(
            "config::load_validated_config invalid configuration from {}",
            source.description()
        )
    })?;
    Ok(cfg)
}

/// Ensure a single, consistent location for config persistence.
pub fn save_config(cfg: &Config) -> Result<()> {
    let source = resolve_config_source(None)?;
    let path = source.path();
    let io_policy = BlockingIoPolicy::bootstrap_defaults();
    if let Some(parent) = path.parent() {
        run_with_policy(
            "config::save_config create parent directory",
            &io_policy,
            CancellationFlag::none(),
            || {
                fs::create_dir_all(parent).with_context(|| {
                    format!(
                        "config::save_config failed creating config directory {:?}",
                        parent
                    )
                })
            },
        )?;
    }
    let raw = toml::to_string_pretty(cfg)
        .context("config::save_config failed to serialize config to TOML")?;
    run_with_policy(
        "config::save_config write config file",
        &io_policy,
        CancellationFlag::none(),
        || {
            fs::write(path, raw.as_bytes()).with_context(|| {
                format!(
                    "config::save_config failed writing config file at {:?}",
                    path
                )
            })
        },
    )?;
    Ok(())
}

/// Allow callers to load configs from non-standard locations.
pub fn load_from_path(path: &Path) -> Result<Config> {
    let (cfg, _) = load_with_source(Some(path))?;
    Ok(cfg)
}

/// Enforce fail-fast startup semantics even when config source is overridden.
pub fn load_validated_from_path(path: &Path) -> Result<Config> {
    let (cfg, source) = load_with_source(Some(path))?;
    validate(&cfg).with_context(|| {
        format!(
            "config::load_validated_from_path invalid configuration from {}",
            source.description()
        )
    })?;
    Ok(cfg)
}
