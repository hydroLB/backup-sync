use anyhow::{Context, Result};
use backup_core::logging::{cid, redact_text};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::error;

mod commands;
mod error;
mod prompt;

/// Summary: Start the CLI entrypoint and dispatch commands.
///
/// Inputs: process arguments parsed by `Args::parse`.
///
/// Outputs: exits the process with deterministic status for success or failure.
///
/// Side effects: Sets a panic hook and writes errors to stderr.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `run` for command dispatch and error reporting.
///
/// Why this exists: centralize CLI startup with consistent error logging.
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
struct Args {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    Status,
    Doctor,
    /// Summary: Generate a new at-rest encryption key file for blob encryption.
    ///
    /// Inputs: Optional output path and an overwrite flag.
    ///
    /// Outputs: Writes a key file and prints its key_id and config guidance.
    ///
    /// Side effects: Writes a key file to disk.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: `[encryption]` config block used by the versioned blob store.
    ///
    /// Why this exists: encrypted backups are only recoverable with the same key; generation should be explicit.
    Keygen {
        /// Summary: Override the key file output path.
        ///
        /// Inputs: CLI flag value.
        ///
        /// Outputs: Uses the provided path instead of the platform default.
        ///
        /// Side effects: None.
        ///
        /// Error handling: Propagates contextual errors to the caller when operations fail.
        ///
        /// Ties to other methods: `commands::key::keygen` key path selection.
        ///
        /// Why this exists: allow custom storage locations for key material.
        #[arg(long)]
        path: Option<PathBuf>,
        /// Summary: Allow overwriting an existing key file.
        ///
        /// Inputs: CLI flag value.
        ///
        /// Outputs: Enables overwrite behavior.
        ///
        /// Side effects: Can overwrite an existing key file.
        ///
        /// Error handling: Propagates contextual errors to the caller when operations fail.
        ///
        /// Ties to other methods: `commands::key::keygen` force handling.
        ///
        /// Why this exists: support intentional key rotation or recovery from partial key files.
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    RunOnce {
        /// Summary: Build a plan without copying files.
        ///
        /// Inputs: CLI flag value.
        ///
        /// Outputs: Enables simulation mode for the run-once command.
        ///
        /// Side effects: None.
        ///
        /// Error handling: Propagates contextual errors to the caller when operations fail.
        ///
        /// Ties to other methods: `commands::run::run_once` simulation handling.
        ///
        /// Why this exists: allow safe previews before executing versioned backups.
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    /// Summary: Guided first-time setup that writes config and can run a backup.
    ///
    /// Inputs: CLI invocation for init.
    ///
    /// Outputs: Creates config and optional initial backup.
    ///
    /// Side effects: Writes config to disk and may trigger a backup.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: `commands::init::init_wizard`.
    ///
    /// Why this exists: streamline first-time setup with validated defaults.
    Init,
    /// Summary: Verify stored backups by re-hashing the latest copy for each file.
    ///
    /// Inputs: CLI invocation for verify.
    ///
    /// Outputs: Reports verification status to stdout.
    ///
    /// Side effects: Reads backup data for hashing and logs status output.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: `commands::verify::verify_backups`.
    ///
    /// Why this exists: surface integrity issues and confirm backup health.
    Verify,
    /// Summary: Install a background service definition.
    ///
    /// Inputs: CLI flags controlling service installation.
    ///
    /// Outputs: Writes a service manifest and optional enablement output.
    ///
    /// Side effects: Writes service manifests and may enable a system service.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: `commands::service::install_service`.
    ///
    /// Why this exists: make daemon installation repeatable across platforms.
    InstallService {
        /// Summary: Select user service mode when applicable.
        ///
        /// Inputs: CLI flag value.
        ///
        /// Outputs: Enables user or system service install path.
        ///
        /// Side effects: None.
        ///
        /// Error handling: Propagates contextual errors to the caller when operations fail.
        ///
        /// Ties to other methods: service manifest selection in `commands::service::install_service`.
        ///
        /// Why this exists: allow platform-appropriate service installation.
        #[arg(long, default_value_t = true)]
        user: bool,
        /// Summary: Override the service manifest output path.
        ///
        /// Inputs: CLI flag value.
        ///
        /// Outputs: Changes where the manifest is written.
        ///
        /// Side effects: None.
        ///
        /// Error handling: Propagates contextual errors to the caller when operations fail.
        ///
        /// Ties to other methods: `commands::service::install_service` output handling.
        ///
        /// Why this exists: support custom install locations when needed.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Summary: Provide an explicit log file path for the service.
        ///
        /// Inputs: CLI flag value.
        ///
        /// Outputs: Configures log path in generated manifests.
        ///
        /// Side effects: None.
        ///
        /// Error handling: Propagates contextual errors to the caller when operations fail.
        ///
        /// Ties to other methods: service manifest generation and logging setup.
        ///
        /// Why this exists: allow service logs to live in a known location.
        #[arg(long)]
        log_path: Option<PathBuf>,
        /// Summary: Print manifest to stdout instead of writing.
        ///
        /// Inputs: CLI flag value.
        ///
        /// Outputs: Emits manifest text to stdout.
        ///
        /// Side effects: Writes to stdout.
        ///
        /// Error handling: Propagates contextual errors to the caller when operations fail.
        ///
        /// Ties to other methods: manifest generation in `commands::service::install_service`.
        ///
        /// Why this exists: enable preview and scripting workflows.
        #[arg(long, default_value_t = false)]
        print: bool,
        /// Summary: Enable or start the service after writing the manifest.
        ///
        /// Inputs: CLI flag value.
        ///
        /// Outputs: Triggers enablement attempts.
        ///
        /// Side effects: Executes service control commands.
        ///
        /// Error handling: Propagates contextual errors to the caller when operations fail.
        ///
        /// Ties to other methods: service control logic in `commands::service::install_service`.
        ///
        /// Why this exists: reduce manual steps after install.
        #[arg(long, default_value_t = false)]
        enable: bool,
        /// Summary: Perform a dry run without writing or enabling.
        ///
        /// Inputs: CLI flag value.
        ///
        /// Outputs: Skips writes and service enablement.
        ///
        /// Side effects: None.
        ///
        /// Error handling: Propagates contextual errors to the caller when operations fail.
        ///
        /// Ties to other methods: `commands::service::install_service` dry run handling.
        ///
        /// Why this exists: allow safe previews of service output.
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
}

/// Summary: Returns whether a CLI command requires a valid startup config before execution.
///
/// Inputs: the parsed CLI command.
///
/// Outputs: `true` when config preflight is required.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: startup preflight in `run`.
///
/// Why this exists: setup commands should stay available even when the current config is broken.
fn command_requires_valid_config(cmd: &Command) -> bool {
    !matches!(cmd, Command::Init | Command::Keygen { .. })
}

/// Summary: Dispatches CLI subcommands based on parsed arguments.
///
/// Inputs: none, uses parsed CLI args.
///
/// Outputs: `Ok(())` after the selected command completes.
///
/// Side effects: Executes the selected command and prints output to stdout/stderr.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: command handlers in the `commands` module.
///
/// Why this exists: keep CLI command routing in one place.
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
