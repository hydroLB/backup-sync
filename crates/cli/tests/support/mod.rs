use backup_core::config::model::{WatchedKind, WatchedPath};
use backup_core::config::registry::config_defaults;
use std::process::{Command, Output};
use std::sync::{Mutex, MutexGuard, OnceLock};
use tempfile::TempDir;

/// Env vars are process-global and must be isolated for deterministic runs.
pub fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

/// Keep CLI tests independent from user machine state and existing config.
pub struct CliFixture {
    _tmp: TempDir,
    config_path: std::path::PathBuf,
    xdg_config_home: std::path::PathBuf,
    home_dir: std::path::PathBuf,
}

impl CliFixture {
    /// Provide stable preconditions for CLI contract tests.
    pub fn new(name: &str) -> Self {
        let tmp = tempfile::Builder::new()
            .prefix(name)
            .tempdir()
            .expect("cli::tests::support::CliFixture::new failed to create tempdir");

        let xdg_config_home = tmp.path().join("xdg-config");
        let home_dir = tmp.path().join("home");
        let app_config_dir = xdg_config_home.join("backup_sync");
        let backup_root = tmp.path().join("backup-root");
        let watched_dir = tmp.path().join("watched");
        let watched_file = watched_dir.join("note.txt");

        std::fs::create_dir_all(&app_config_dir)
            .expect("cli::tests::support::CliFixture::new failed to create app config dir");
        std::fs::create_dir_all(&backup_root)
            .expect("cli::tests::support::CliFixture::new failed to create backup root");
        std::fs::create_dir_all(&watched_dir)
            .expect("cli::tests::support::CliFixture::new failed to create watched dir");
        std::fs::create_dir_all(home_dir.join(".config/backup_sync"))
            .expect("cli::tests::support::CliFixture::new failed to create home config dir");
        std::fs::create_dir_all(home_dir.join("Library/Application Support/backup_sync"))
            .expect("cli::tests::support::CliFixture::new failed to create macOS app support dir");
        std::fs::write(&watched_file, "fixture-data\n")
            .expect("cli::tests::support::CliFixture::new failed to write watched file");

        let mut cfg =
            config_defaults().expect("cli::tests::support::CliFixture::new config_defaults failed");
        cfg.backup_root = backup_root.clone();
        if let Some(destination) = cfg.destinations.get_mut(0) {
            destination.path = backup_root;
            destination.id = "default".to_string();
        }
        cfg.watched = vec![WatchedPath {
            path: watched_file,
            kind: WatchedKind::File,
            enabled: true,
            destination_id: "default".to_string(),
            max_backups_per_file: None,
        }];

        let config_path = app_config_dir.join("config.toml");
        let raw_config = toml::to_string_pretty(&cfg)
            .expect("cli::tests::support::CliFixture::new failed to serialize config");
        std::fs::write(&config_path, raw_config)
            .expect("cli::tests::support::CliFixture::new failed to write config.toml");

        Self {
            _tmp: tmp,
            config_path,
            xdg_config_home,
            home_dir,
        }
    }

    /// Centralize environment contract for all CLI integration tests.
    pub fn command(&self) -> Command {
        let exe = std::env::var_os("CARGO_BIN_EXE_cli").unwrap_or_else(|| {
            let current = std::env::current_exe()
                .expect("cli::tests::support::CliFixture::command failed to resolve current_exe");
            current
                .parent()
                .and_then(std::path::Path::parent)
                .map(|dir| dir.join(format!("cli{}", std::env::consts::EXE_SUFFIX)))
                .expect("cli::tests::support::CliFixture::command failed to derive binary path")
                .into_os_string()
        });
        let mut command = Command::new(exe);
        command.env("BACKUP_SYNC_CONFIG", &self.config_path);
        command.env("XDG_CONFIG_HOME", &self.xdg_config_home);
        command.env("HOME", &self.home_dir);
        command
    }
}

/// Keep command invocation consistent and reusable across tests.
pub fn run_cli(fixture: &CliFixture, args: &[&str]) -> Output {
    fixture
        .command()
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("cli::tests::support::run_cli execution failed: {error}"))
}
