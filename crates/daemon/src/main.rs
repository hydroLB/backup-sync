use anyhow::{Context, Result};
use backup_core::{
    load_validated_config, load_validated_from_path, logging::redact_text, platform::paths,
    StateStore,
};
use chrono::Utc;
use daemon::init::{ensure_parent_dir, init_logging, validate_environment, DaemonArgs};
use daemon::runtime::{cid, run_daemon};
use tracing::error;

mod error;

/// Summary: Entry point for the daemon process.
///
/// Inputs: the process environment and on disk config and state.
///
/// Outputs: exits the process with deterministic status for success or failure.
///
/// Side effects: Initializes logging, reads config/state, and starts runtime tasks.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: config loading, logging setup, and runtime loop startup.
///
/// Why this exists: orchestrate daemon startup with clear error context.
#[tokio::main]
async fn main() {
    let panic_cid = cid("daemon-panic");
    std::panic::set_hook(Box::new(move |info| {
        let mapped = error::map_panic(&info.to_string());
        error!(
            cid = %panic_cid,
            action = "panic",
            code = %mapped.code.as_str(),
            retryable = mapped.retryable,
            hint = %mapped.hint,
            panic = %mapped.message,
            "daemon panic"
        );
    }));
    if let Err(err) = run_main().await {
        let mapped = error::map_anyhow(&err, "daemon::main startup/runtime failure");
        let correlation_id = cid("daemon-main");
        error!(
            cid = %correlation_id,
            action = "process_failed",
            code = %mapped.code.as_str(),
            retryable = mapped.retryable,
            exit_code = mapped.exit_code(),
            hint = %mapped.hint,
            error = %redact_text(&mapped.message),
            "daemon process failed"
        );
        eprintln!("{}", mapped.render_for_user());
        std::process::exit(mapped.exit_code());
    }
}

/// Summary: Executes daemon bootstrap and runtime orchestration.
///
/// Inputs: process arguments, environment, config, and persisted state.
///
/// Outputs: `Ok(())` when daemon exits cleanly.
///
/// Side effects: Initializes logging and starts daemon runtime tasks.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon process boundary handler in `main`.
///
/// Why this exists: isolate fallible startup logic for centralized boundary mapping.
async fn run_main() -> Result<()> {
    let args = DaemonArgs::from_env();
    let env = validate_environment().context("daemon::main failed environment validation")?;
    if let Some(custom_log) = &args.log_path {
        ensure_parent_dir(custom_log, "log override")
            .context("daemon::main failed to prepare log override directory")?;
    }
    let log_path = args.log_path.clone().or(Some(env.log_path));
    init_logging(log_path).context("daemon::main failed to initialize logging")?;
    let cfg = match args.config_path.clone() {
        Some(path) => load_validated_from_path(&path)
            .context("daemon::main failed to load configuration from override path")?,
        None => load_validated_config().context("daemon::main failed to load configuration")?,
    };
    let state_path =
        paths::state_file_path().context("daemon::main failed to resolve state path")?;
    let (mut state, store) = StateStore::load_or_default(state_path)
        .context("daemon::main failed to load persisted daemon state")?;
    state.start_ts = Some(Utc::now().timestamp());
    state.version = Some(env!("CARGO_PKG_VERSION").to_string());
    run_daemon(cfg, state, store)
        .await
        .context("daemon::main runtime execution failed")
}
