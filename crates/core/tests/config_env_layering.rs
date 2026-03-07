use backup_core::config::registry::config_defaults;
use backup_core::load_validated_config;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

const ENV_CONFIG_PATH: &str = "BACKUP_SYNC_CONFIG";
const ENV_INTERVAL_SECONDS: &str = "BACKUP_SYNC_INTERVAL_SECONDS";
const ENV_SAFE_MODE: &str = "BACKUP_SYNC_SAFE_MODE";

/// Summary: Provides a process-wide lock for env var mutation in tests.
///
/// Inputs: none.
///
/// Outputs: a static mutex guarding env changes.
///
/// Side effects: Initializes a static lock once.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: env override tests in this module.
///
/// Why this exists: env vars are process-global and these tests must not run concurrently.
fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Summary: RAII guard that restores mutated env vars after a test.
///
/// Inputs: list of env variable names captured before mutation.
///
/// Outputs: a guard restoring prior values on drop.
///
/// Side effects: Mutates process environment during the test lifetime.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: all env layering tests in this file.
///
/// Why this exists: keep tests deterministic and isolated despite process-global env state.
struct EnvGuard {
    saved: Vec<(&'static str, Option<OsString>)>,
}

impl EnvGuard {
    /// Summary: Applies env overrides and captures prior values for restoration.
    ///
    /// Inputs: env overrides where `None` removes a variable.
    ///
    /// Outputs: an `EnvGuard` restoring previous values on drop.
    ///
    /// Side effects: Mutates process environment variables.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: env layering tests that require temporary overrides.
    ///
    /// Why this exists: reduce duplicated env mutation and cleanup code in tests.
    fn apply(overrides: &[(&'static str, Option<&str>)]) -> Self {
        let mut saved = Vec::with_capacity(overrides.len());
        for (name, value) in overrides {
            saved.push((*name, std::env::var_os(name)));
            match value {
                Some(raw) => std::env::set_var(name, raw),
                None => std::env::remove_var(name),
            }
        }
        Self { saved }
    }
}

impl Drop for EnvGuard {
    /// Summary: Restores previously captured env vars.
    ///
    /// Inputs: none.
    ///
    /// Outputs: none.
    ///
    /// Side effects: Mutates process environment variables.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: `EnvGuard::apply`.
    ///
    /// Why this exists: guarantee env isolation across tests even on assertion failures.
    fn drop(&mut self) {
        for (name, prior) in &self.saved {
            match prior {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
}

/// Summary: Writes a baseline valid config file at the provided path.
///
/// Inputs: target config file path.
///
/// Outputs: none.
///
/// Side effects: Writes TOML config content to disk.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: env layering tests that load config from explicit paths.
///
/// Why this exists: keep test setup deterministic and focused on env-layering behavior.
fn write_valid_config(path: &Path) {
    let mut cfg = config_defaults().expect("config_env_layering::write_valid_config defaults");
    let backup_root = path
        .parent()
        .expect("config_env_layering::write_valid_config parent")
        .join("backup-root");
    cfg.backup_root = backup_root.clone();
    if let Some(dest) = cfg.destinations.get_mut(0) {
        dest.path = backup_root;
    }

    let raw = toml::to_string_pretty(&cfg)
        .expect("config_env_layering::write_valid_config serialize config");
    fs::write(path, raw).expect("config_env_layering::write_valid_config write config");
}

#[test]
/// Summary: Ensures env-layered interval overrides are applied during validated startup load.
///
/// Inputs: a valid config file plus interval env override.
///
/// Outputs: config with overridden interval.
///
/// Side effects: Writes temp config and mutates env variables for the test scope.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `load_validated_config` env layering.
///
/// Why this exists: verify startup behavior is deterministic when env overrides are present.
fn load_validated_config_applies_env_interval_override() {
    let _env_guard_lock = env_lock().lock().expect(
        "config_env_layering::load_validated_config_applies_env_interval_override env lock",
    );
    let dir = tempfile::tempdir()
        .expect("config_env_layering::load_validated_config_applies_env_interval_override tempdir");
    let cfg_path = dir.path().join("config.toml");
    write_valid_config(&cfg_path);

    let _env_guard = EnvGuard::apply(&[
        (ENV_CONFIG_PATH, Some(cfg_path.to_string_lossy().as_ref())),
        (ENV_INTERVAL_SECONDS, Some("120")),
        (ENV_SAFE_MODE, None),
    ]);

    let cfg = load_validated_config()
        .expect("config_env_layering::load_validated_config_applies_env_interval_override load");
    assert_eq!(cfg.interval_seconds, 120);
}

#[test]
/// Summary: Ensures invalid env-layered interval values fail fast with consistent startup context.
///
/// Inputs: a valid config file plus an invalid interval env override.
///
/// Outputs: load failure containing standardized invalid-config context.
///
/// Side effects: Writes temp config and mutates env variables for the test scope.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `load_validated_config` validation context formatting.
///
/// Why this exists: keep invalid config errors consistent across CLI, daemon, and GUI startup.
fn load_validated_config_reports_consistent_invalid_config_context() {
    let _env_guard_lock = env_lock().lock().expect(
        "config_env_layering::load_validated_config_reports_consistent_invalid_config_context env lock",
    );
    let dir = tempfile::tempdir().expect(
        "config_env_layering::load_validated_config_reports_consistent_invalid_config_context tempdir",
    );
    let cfg_path = dir.path().join("config.toml");
    write_valid_config(&cfg_path);

    let _env_guard = EnvGuard::apply(&[
        (ENV_CONFIG_PATH, Some(cfg_path.to_string_lossy().as_ref())),
        (ENV_INTERVAL_SECONDS, Some("1")),
        (ENV_SAFE_MODE, None),
    ]);

    let error = load_validated_config().expect_err(
        "config_env_layering::load_validated_config_reports_consistent_invalid_config_context expected failure",
    );
    let msg = format!("{error:#}");
    assert!(
        msg.contains("config::load_validated_config invalid configuration"),
        "expected consistent invalid config context, got: {}",
        msg
    );
    assert!(
        msg.contains("interval_seconds must be >="),
        "expected interval guardrail details, got: {}",
        msg
    );
}

#[test]
/// Summary: Ensures malformed boolean env overrides fail with clear parse errors.
///
/// Inputs: a valid config file plus malformed safe-mode env override.
///
/// Outputs: load failure containing env variable parse context.
///
/// Side effects: Writes temp config and mutates env variables for the test scope.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `parse_env_bool` and `load_validated_config`.
///
/// Why this exists: malformed env values should fail fast with actionable diagnostics.
fn load_validated_config_rejects_malformed_safe_mode_env() {
    let _env_guard_lock = env_lock().lock().expect(
        "config_env_layering::load_validated_config_rejects_malformed_safe_mode_env env lock",
    );
    let dir = tempfile::tempdir().expect(
        "config_env_layering::load_validated_config_rejects_malformed_safe_mode_env tempdir",
    );
    let cfg_path = dir.path().join("config.toml");
    write_valid_config(&cfg_path);

    let _env_guard = EnvGuard::apply(&[
        (ENV_CONFIG_PATH, Some(cfg_path.to_string_lossy().as_ref())),
        (ENV_INTERVAL_SECONDS, None),
        (ENV_SAFE_MODE, Some("not-a-bool")),
    ]);

    let error = load_validated_config().expect_err(
        "config_env_layering::load_validated_config_rejects_malformed_safe_mode_env expected failure",
    );
    let msg = format!("{error:#}");
    assert!(
        msg.contains("config::env_layering invalid boolean for BACKUP_SYNC_SAFE_MODE"),
        "expected env parse context, got: {}",
        msg
    );
}
