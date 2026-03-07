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
    /// Summary: Returns the filesystem path used by this config source.
    ///
    /// Inputs: none.
    ///
    /// Outputs: a config file path reference.
    ///
    /// Side effects: None.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: config source resolution and load diagnostics.
    ///
    /// Why this exists: centralize source path rendering for consistent error messages.
    fn path(&self) -> &Path {
        match self {
            Self::DefaultPath(path) => path,
            Self::ExplicitPath { path, .. } => path,
        }
    }

    /// Summary: Returns whether this source is the default config path.
    ///
    /// Inputs: none.
    ///
    /// Outputs: true when source is default path, false otherwise.
    ///
    /// Side effects: None.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: config load behavior for first-run default creation.
    ///
    /// Why this exists: explicit paths must fail if missing while default path may be bootstrapped.
    fn is_default_path(&self) -> bool {
        matches!(self, Self::DefaultPath(_))
    }

    /// Summary: Builds a stable description string for source-aware validation errors.
    ///
    /// Inputs: none.
    ///
    /// Outputs: a human-readable source label.
    ///
    /// Side effects: None.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: load and validation error formatting.
    ///
    /// Why this exists: keep invalid-config diagnostics consistent across all entrypoints.
    fn description(&self) -> String {
        match self {
            Self::DefaultPath(path) => format!("default path {:?}", path),
            Self::ExplicitPath { path, reason } => {
                format!("explicit path {:?} ({reason})", path)
            }
        }
    }
}

/// Summary: Resolves the effective config source with env layering support.
///
/// Inputs: an optional explicit path override.
///
/// Outputs: the resolved `ConfigSource`.
///
/// Side effects: Reads process environment variables.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: unified load path used by daemon, CLI, and GUI.
///
/// Why this exists: ensure every entrypoint resolves config location consistently.
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

/// Summary: Reads a UTF-8 env variable and skips empty values.
///
/// Inputs: an env variable name.
///
/// Outputs: `Some(value)` for non-empty values, otherwise `None`.
///
/// Side effects: Reads process environment variables.
///
/// Error handling: Returns a contextual error when env content is non-UTF-8.
///
/// Ties to other methods: env layering for startup config.
///
/// Why this exists: centralize parsing and error shape for config env overrides.
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

/// Summary: Parses an unsigned integer env override for config layering.
///
/// Inputs: env variable name and raw string value.
///
/// Outputs: parsed `u64`.
///
/// Side effects: None.
///
/// Error handling: Returns a descriptive parse error tied to the env variable.
///
/// Ties to other methods: startup env layering for numeric config knobs.
///
/// Why this exists: keep env parse diagnostics consistent across entrypoints.
fn parse_env_u64(name: &str, raw: &str) -> Result<u64> {
    raw.parse::<u64>().with_context(|| {
        format!("config::env_layering invalid unsigned integer for {name}: {raw:?}")
    })
}

/// Summary: Parses a boolean env override for config layering.
///
/// Inputs: env variable name and raw string value.
///
/// Outputs: parsed boolean value.
///
/// Side effects: None.
///
/// Error handling: Returns a descriptive parse error tied to the env variable.
///
/// Ties to other methods: startup env layering for safety-related config knobs.
///
/// Why this exists: support operational toggles without ambiguous boolean parsing.
fn parse_env_bool(name: &str, raw: &str) -> Result<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => bail!(
            "config::env_layering invalid boolean for {name}: {raw:?} (accepted: true/false/1/0/yes/no/on/off)"
        ),
    }
}

/// Summary: Applies env-layered startup overrides on top of loaded file config.
///
/// Inputs: a loaded config.
///
/// Outputs: config after env overrides are applied.
///
/// Side effects: Reads process environment variables.
///
/// Error handling: Returns descriptive errors for malformed env values.
///
/// Ties to other methods: unified startup config loading for daemon, CLI, and GUI.
///
/// Why this exists: allow deterministic operational overrides without editing config files.
fn apply_env_layering(mut cfg: Config) -> Result<Config> {
    if let Some(raw) = read_non_empty_env(ENV_INTERVAL_SECONDS)? {
        cfg.interval_seconds = parse_env_u64(ENV_INTERVAL_SECONDS, &raw)?;
    }

    if let Some(raw) = read_non_empty_env(ENV_SAFE_MODE)? {
        cfg.safe_mode = parse_env_bool(ENV_SAFE_MODE, &raw)?;
    }

    Ok(cfg)
}

/// Summary: Loads configuration from the resolved source.
///
/// Inputs: a resolved config source.
///
/// Outputs: a normalized config value.
///
/// Side effects: Reads config file from disk and may write first-run defaults.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: all config load entrypoints and startup validation.
///
/// Why this exists: unify file loading behavior before env layering and validation.
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

/// Summary: Loads config and returns source metadata for validation/error reporting.
///
/// Inputs: optional explicit path override.
///
/// Outputs: `(Config, ConfigSource)` after env layering.
///
/// Side effects: Reads and optionally writes config files, reads env variables.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: public load and load+validate APIs.
///
/// Why this exists: keep source-aware diagnostics consistent and avoid duplicated load logic.
fn load_with_source(path_override: Option<&Path>) -> Result<(Config, ConfigSource)> {
    let source = resolve_config_source(path_override)?;
    let cfg = load_from_source(&source)?;
    let cfg = apply_env_layering(cfg)?;
    Ok((cfg, source))
}

/// Summary: Loads the user config from the default source with env layering.
///
/// Inputs: none.
///
/// Outputs: a parsed `Config` after env layering.
///
/// Side effects: Reads config files and may write a default config to disk.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: CLI, daemon, and GUI startup workflows.
///
/// Why this exists: provide a single source of truth for config loading behavior.
pub fn load_config() -> Result<Config> {
    let (cfg, _) = load_with_source(None)?;
    Ok(cfg)
}

/// Summary: Loads and validates config from the default source with env layering.
///
/// Inputs: none.
///
/// Outputs: a validated `Config` value.
///
/// Side effects: Reads config files and may write a default config to disk.
///
/// Error handling: Returns consistent invalid-config errors with source context.
///
/// Ties to other methods: daemon, CLI, and GUI startup preflight checks.
///
/// Why this exists: ensure all entrypoints fail fast on invalid config with the same error model.
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

/// Summary: Saves the config to the default path on disk.
///
/// Inputs: a reference to the config to persist.
///
/// Outputs: `Ok(())` when the file is written.
///
/// Side effects: Creates config directories and writes the config file to disk.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: config initialization and UI driven updates.
///
/// Why this exists: ensure a single, consistent location for config persistence.
pub fn save_config(cfg: &Config) -> Result<()> {
    let path = config_file_path()?;
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
            fs::write(&path, raw.as_bytes()).with_context(|| {
                format!(
                    "config::save_config failed writing config file at {:?}",
                    path
                )
            })
        },
    )?;
    Ok(())
}

/// Summary: Loads a config from an explicit path with env layering.
///
/// Inputs: the filesystem path to the config file.
///
/// Outputs: a parsed `Config` value.
///
/// Side effects: Reads the config file from disk.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: diagnostics and tests that load configs from fixtures.
///
/// Why this exists: allow callers to load configs from non-standard locations.
pub fn load_from_path(path: &Path) -> Result<Config> {
    let (cfg, _) = load_with_source(Some(path))?;
    Ok(cfg)
}

/// Summary: Loads and validates config from an explicit path with env layering.
///
/// Inputs: filesystem path to the config file.
///
/// Outputs: a validated `Config` value.
///
/// Side effects: Reads the config file from disk and reads env overrides.
///
/// Error handling: Returns consistent invalid-config errors with source context.
///
/// Ties to other methods: explicit-path startup flows and tests.
///
/// Why this exists: enforce fail-fast startup semantics even when config source is overridden.
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
