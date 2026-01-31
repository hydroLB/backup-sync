use anyhow::{Context, Result};
use backup_core::{load_config, load_from_path, platform::paths, validate, StateStore};
use chrono::Utc;
use daemon::init::{args::DaemonArgs, environment, logging_setup};
use daemon::runtime::run_daemon;

/// Purpose: Entry point for the daemon process.
///
/// Inputs: the process environment and on disk config and state.
/// Outputs: `Ok(())` when the daemon exits cleanly.
/// Ties to: config loading, logging setup, and runtime loop startup.
/// Side effects: Initializes logging, reads config/state, and starts runtime tasks.
/// Why: orchestrate daemon startup with clear error context.
#[tokio::main]
async fn main() -> Result<()> {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("[daemon panic] {info}");
    }));
    let args = DaemonArgs::from_env();
    let env = environment::validate_environment()
        .context("daemon::main failed environment validation")?;
    if let Some(custom_log) = &args.log_path {
        environment::ensure_parent_dir(custom_log, "log override")
            .context("daemon::main failed to prepare log override directory")?;
    }
    let log_path = args.log_path.clone().or(Some(env.log_path));
    logging_setup::init_logging(log_path).context("daemon::main failed to initialize logging")?;
    let cfg = match args.config_path.clone() {
        Some(path) => load_from_path(&path)
            .context("daemon::main failed to load configuration from override path")?,
        None => load_config().context("daemon::main failed to load configuration")?,
    };
    validate(&cfg).context("daemon::main configuration validation failed")?;
    let state_path =
        paths::state_file_path().context("daemon::main failed to resolve state path")?;
    let (mut state, store) = StateStore::load_or_default(state_path)
        .context("daemon::main failed to load persisted daemon state")?;
    state.start_ts = Some(Utc::now().timestamp());
    state.version = Some(env!("CARGO_PKG_VERSION").to_string());
    if let Err(e) = run_daemon(cfg, state, store).await {
        eprintln!("[daemon error] {e:?}");
        return Err(e);
    }
    Ok(())
}
