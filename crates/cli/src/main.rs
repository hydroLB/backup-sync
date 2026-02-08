use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod commands;
mod prompt;

/// Purpose: Start the CLI entrypoint and dispatch commands.
///
/// Inputs: process arguments parsed by `Args::parse`.
/// Outputs: `Ok(())` when the selected command completes.
/// Ties to: `run` for command dispatch and error reporting.
/// Side effects: Sets a panic hook and writes errors to stderr.
/// Why: centralize CLI startup with consistent error logging.
#[tokio::main]
async fn main() -> Result<()> {
    // Surface panics/errors to stderr so CLI failures are visible in automation.
    std::panic::set_hook(Box::new(|info| {
        eprintln!("[cli panic] {info}");
    }));
    let result = run().await;
    if let Err(ref err) = result {
        eprintln!("[cli error] {err:?}");
    }
    result
}

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    Status,
    Doctor,
    /// Purpose: Generate a new at-rest encryption key file for blob encryption.
    ///
    /// Inputs: Optional output path and an overwrite flag.
    /// Outputs: Writes a key file and prints its key_id and config guidance.
    /// Ties to: `[encryption]` config block used by the versioned blob store.
    /// Side effects: Writes a key file to disk.
    /// Why: encrypted backups are only recoverable with the same key; generation should be explicit.
    Keygen {
        /// Purpose: Override the key file output path.
        ///
        /// Inputs: CLI flag value.
        /// Outputs: Uses the provided path instead of the platform default.
        /// Ties to: `commands::key::keygen` key path selection.
        /// Side effects: None.
        /// Why: allow custom storage locations for key material.
        #[arg(long)]
        path: Option<PathBuf>,
        /// Purpose: Allow overwriting an existing key file.
        ///
        /// Inputs: CLI flag value.
        /// Outputs: Enables overwrite behavior.
        /// Ties to: `commands::key::keygen` force handling.
        /// Side effects: Can overwrite an existing key file.
        /// Why: support intentional key rotation or recovery from partial key files.
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    RunOnce {
        /// Purpose: Build a plan without copying files.
        ///
        /// Inputs: CLI flag value.
        /// Outputs: Enables simulation mode for the run-once command.
        /// Ties to: `commands::run::run_once` simulation handling.
        /// Side effects: None.
        /// Why: allow safe previews before executing versioned backups.
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    /// Purpose: Guided first-time setup that writes config and can run a backup.
    ///
    /// Inputs: CLI invocation for init.
    /// Outputs: Creates config and optional initial backup.
    /// Ties to: `commands::init::init_wizard`.
    /// Side effects: Writes config to disk and may trigger a backup.
    /// Why: streamline first-time setup with validated defaults.
    Init,
    /// Purpose: Verify stored backups by re-hashing the latest copy for each file.
    ///
    /// Inputs: CLI invocation for verify.
    /// Outputs: Reports verification status to stdout.
    /// Ties to: `commands::verify::verify_backups`.
    /// Side effects: Reads backup data for hashing and logs status output.
    /// Why: surface integrity issues and confirm backup health.
    Verify,
    /// Purpose: Install a background service definition.
    ///
    /// Inputs: CLI flags controlling service installation.
    /// Outputs: Writes a service manifest and optional enablement output.
    /// Ties to: `commands::service::install_service`.
    /// Side effects: Writes service manifests and may enable a system service.
    /// Why: make daemon installation repeatable across platforms.
    InstallService {
        /// Purpose: Select user service mode when applicable.
        ///
        /// Inputs: CLI flag value.
        /// Outputs: Enables user or system service install path.
        /// Ties to: service manifest selection in `commands::service::install_service`.
        /// Side effects: None.
        /// Why: allow platform-appropriate service installation.
        #[arg(long, default_value_t = true)]
        user: bool,
        /// Purpose: Override the service manifest output path.
        ///
        /// Inputs: CLI flag value.
        /// Outputs: Changes where the manifest is written.
        /// Ties to: `commands::service::install_service` output handling.
        /// Side effects: None.
        /// Why: support custom install locations when needed.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Purpose: Provide an explicit log file path for the service.
        ///
        /// Inputs: CLI flag value.
        /// Outputs: Configures log path in generated manifests.
        /// Ties to: service manifest generation and logging setup.
        /// Side effects: None.
        /// Why: allow service logs to live in a known location.
        #[arg(long)]
        log_path: Option<PathBuf>,
        /// Purpose: Print manifest to stdout instead of writing.
        ///
        /// Inputs: CLI flag value.
        /// Outputs: Emits manifest text to stdout.
        /// Ties to: manifest generation in `commands::service::install_service`.
        /// Side effects: Writes to stdout.
        /// Why: enable preview and scripting workflows.
        #[arg(long, default_value_t = false)]
        print: bool,
        /// Purpose: Enable or start the service after writing the manifest.
        ///
        /// Inputs: CLI flag value.
        /// Outputs: Triggers enablement attempts.
        /// Ties to: service control logic in `commands::service::install_service`.
        /// Side effects: Executes service control commands.
        /// Why: reduce manual steps after install.
        #[arg(long, default_value_t = false)]
        enable: bool,
        /// Purpose: Perform a dry run without writing or enabling.
        ///
        /// Inputs: CLI flag value.
        /// Outputs: Skips writes and service enablement.
        /// Ties to: `commands::service::install_service` dry run handling.
        /// Side effects: None.
        /// Why: allow safe previews of service output.
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
}

/// Purpose: Dispatches CLI subcommands based on parsed arguments.
///
/// Inputs: none, uses parsed CLI args.
/// Outputs: `Ok(())` after the selected command completes.
/// Ties to: command handlers in the `commands` module.
/// Side effects: Executes the selected command and prints output to stdout/stderr.
/// Why: keep CLI command routing in one place.
async fn run() -> Result<()> {
    let args = Args::parse();
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
