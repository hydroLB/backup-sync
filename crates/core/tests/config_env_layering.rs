use backup_core::config::registry::config_defaults;
use backup_core::{load_config, load_validated_config, save_config};
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

const ENV_CONFIG_PATH: &str = "BACKUP_SYNC_CONFIG";
const ENV_INTERVAL_SECONDS: &str = "BACKUP_SYNC_INTERVAL_SECONDS";
const ENV_SAFE_MODE: &str = "BACKUP_SYNC_SAFE_MODE";

/// Env vars are process-global and these tests must not run concurrently.
fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Keep tests deterministic and isolated despite process-global env state.
struct EnvGuard {
    saved: Vec<(&'static str, Option<OsString>)>,
}

impl EnvGuard {
    /// Reduce duplicated env mutation and cleanup code in tests.
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
    /// Guarantee env isolation across tests even on assertion failures.
    fn drop(&mut self) {
        for (name, prior) in &self.saved {
            match prior {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
}

/// Keep test setup deterministic and focused on env-layering behavior.
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
/// Verify startup behavior is deterministic when env overrides are present.
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
/// Keep invalid config errors consistent across CLI, daemon, and GUI startup.
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
/// Malformed env values should fail fast with actionable diagnostics.
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

#[test]
/// Saving must honor the same explicit env path that subsequent loads resolve.
fn save_config_persists_to_env_override_and_round_trips() {
    let _env_guard_lock = env_lock().lock().expect(
        "config_env_layering::save_config_persists_to_env_override_and_round_trips env lock",
    );
    let dir = tempfile::tempdir().expect(
        "config_env_layering::save_config_persists_to_env_override_and_round_trips tempdir",
    );
    let cfg_path = dir.path().join("nested").join("config.toml");
    let _env_guard = EnvGuard::apply(&[
        (ENV_CONFIG_PATH, Some(cfg_path.to_string_lossy().as_ref())),
        (ENV_INTERVAL_SECONDS, None),
        (ENV_SAFE_MODE, None),
    ]);

    let mut cfg = config_defaults().expect(
        "config_env_layering::save_config_persists_to_env_override_and_round_trips defaults",
    );
    cfg.interval_seconds = 777;
    save_config(&cfg)
        .expect("config_env_layering::save_config_persists_to_env_override_and_round_trips save");

    assert!(
        cfg_path.is_file(),
        "expected config to be saved at BACKUP_SYNC_CONFIG"
    );
    let loaded = load_config()
        .expect("config_env_layering::save_config_persists_to_env_override_and_round_trips load");
    assert_eq!(loaded.interval_seconds, 777);
}
