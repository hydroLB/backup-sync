use anyhow::{Context, Result};
use backup_core::logging::{cid, redact_text};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::error;

mod commands;
mod error;
mod prompt;

/// Centralize CLI startup with consistent error logging.
#[tokio::main]
async fn main() {
    backup_core::logging::init();
    // Surface panics/errors with structured fields so failures are visible in automation.
    let panic_cid = cid("cli-panic");
    std::panic::set_hook(Box::new(move |info| {
        let mapped = error::map_panic(&info.to_string());
        error!(
            cid = %panic_cid,
            action = "panic",
            code = %mapped.code.as_str(),
            retryable = mapped.retryable,
            hint = %mapped.hint,
            panic = %mapped.message,
            "cli panic"
        );
    }));
    if let Err(err) = run().await {
        let mapped = error::map_anyhow(&err, "cli::main command failed");
        let command_cid = cid("cli-main");
        error!(
            cid = %command_cid,
            action = "command_failed",
            code = %mapped.code.as_str(),
            retryable = mapped.retryable,
            exit_code = mapped.exit_code(),
            hint = %mapped.hint,
            error = %redact_text(&mapped.message),
            "cli command failed"
        );
        eprintln!("{}", mapped.render_for_user());
        std::process::exit(mapped.exit_code());
    }
}

#[derive(Parser)]
#[command(
    name = "backup-sync",
    bin_name = "backup-sync",
    version,
    about = "Versioned, content-addressed backups for local files and folders"
)]
struct Args {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show the latest backup state and configuration summary.
    Status,
    /// Diagnose configuration, destination, and runtime problems.
    Doctor,
    /// Generate a key for encrypted backup blobs.
    Keygen {
        /// Write the key to this path instead of the platform default.
        #[arg(long)]
        path: Option<PathBuf>,
        /// Replace an existing key file at the selected path.
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    /// Run one backup cycle and exit.
    RunOnce {
        /// Preview changes without writing blobs or manifests.
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    /// Create a validated configuration with an interactive setup flow.
    Init,
    /// Verify the latest stored copies against their recorded hashes.
    Verify,
    /// Generate or install the platform background-service definition.
    InstallService {
        /// Install as a user service when the platform supports both scopes.
        #[arg(long, default_value_t = true)]
        user: bool,
        /// Write the service definition to a custom path.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Store service logs at this path.
        #[arg(long)]
        log_path: Option<PathBuf>,
        /// Print the generated definition instead of writing it.
        #[arg(long, default_value_t = false)]
        print: bool,
        /// Enable or start the service after installation.
        #[arg(long, default_value_t = false)]
        enable: bool,
        /// Preview the install without writing or enabling anything.
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
}

/// Setup commands should stay available even when the current config is broken.
fn command_requires_valid_config(cmd: &Command) -> bool {
    !matches!(cmd, Command::Init | Command::Keygen { .. })
}

/// Keep CLI command routing in one place.
async fn run() -> Result<()> {
    let args = Args::parse();
    if command_requires_valid_config(&args.cmd) {
        backup_core::load_validated_config().context("cli::run startup config preflight failed")?;
    }
    match args.cmd {
        Command::Status => commands::status().context("cli::run status command failed")?,
        Command::Doctor => commands::doctor().context("cli::run doctor command failed")?,
        Command::Keygen { path, force } => {
            commands::keygen(path, force).context("cli::run keygen command failed")?
        }
        Command::RunOnce { dry_run } => commands::run::run_once(dry_run)
            .await
            .context("cli::run run-once command failed")?,
        Command::Init => commands::init::init_wizard()
            .await
            .context("cli::run init command failed")?,
        Command::Verify => commands::verify::verify_backups()
            .await
            .context("cli::run verify command failed")?,
        Command::InstallService {
            user,
            output,
            log_path,
            print,
            enable,
            dry_run,
        } => commands::service::install_service(user, output, log_path, print, enable, dry_run)
            .context("cli::run install-service command failed")?,
    }
    Ok(())
}
