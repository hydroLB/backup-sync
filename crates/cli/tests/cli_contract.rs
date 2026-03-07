mod support;

#[test]
/// Summary: Verifies top-level CLI help output exposes the stable public command set.
///
/// Inputs: `--help` invocation.
///
/// Outputs: assertions over command names and usage banner in stdout.
///
/// Side effects: Executes the CLI binary.
///
/// Error handling: Panics with contextual method/file messaging when assertions fail.
///
/// Ties to other methods: clap command wiring in `crates/cli/src/main.rs`.
///
/// Why this exists: protect user-facing command discoverability from accidental regressions.
fn cli_help_contract_lists_supported_commands() {
    let _env_lock = support::env_lock();
    let fixture = support::CliFixture::new("cli-help-contract");

    let output = support::run_cli(&fixture, &["--help"]);
    assert!(
        output.status.success(),
        "cli_contract::cli_help_contract_lists_supported_commands expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("status"));
    assert!(stdout.contains("doctor"));
    assert!(stdout.contains("run-once"));
    assert!(stdout.contains("verify"));
    assert!(stdout.contains("install-service"));
}

#[test]
/// Summary: Verifies `status` command output preserves required line-level response contract.
///
/// Inputs: deterministic fixture config and `status` invocation.
///
/// Outputs: assertions for required status summary lines.
///
/// Side effects: Executes the CLI binary and writes/reads state files in fixture directories.
///
/// Error handling: Panics with contextual method/file messaging when assertions fail.
///
/// Ties to other methods: `crates/cli/src/commands/status.rs` output formatting.
///
/// Why this exists: keep scripting and operator expectations stable for status output.
fn cli_status_contract_includes_required_summary_lines() {
    let _env_lock = support::env_lock();
    let fixture = support::CliFixture::new("cli-status-contract");

    let output = support::run_cli(&fixture, &["status"]);
    assert!(
        output.status.success(),
        "cli_contract::cli_status_contract_includes_required_summary_lines expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("last run:"));
    assert!(stdout.contains("tracked files:"));
    assert!(stdout.contains("backup copies:"));
    assert!(stdout.contains("config root:"));
}

#[test]
/// Summary: Verifies `run-once --dry-run` output keeps stable simulation contract fields.
///
/// Inputs: deterministic fixture config and dry-run invocation.
///
/// Outputs: assertions for simulation output prefixes used by humans and automation.
///
/// Side effects: Executes a read-only simulation scan over fixture content.
///
/// Error handling: Panics with contextual method/file messaging when assertions fail.
///
/// Ties to other methods: `crates/cli/src/commands/run.rs` dry-run output.
///
/// Why this exists: prevent accidental contract drift in dry-run operational reporting.
fn cli_run_once_dry_run_contract_includes_simulation_fields() {
    let _env_lock = support::env_lock();
    let fixture = support::CliFixture::new("cli-run-once-contract");

    let output = support::run_cli(&fixture, &["run-once", "--dry-run"]);
    assert!(
        output.status.success(),
        "cli_contract::cli_run_once_dry_run_contract_includes_simulation_fields expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Simulation:"));
    assert!(stdout.contains("watched="));
    assert!(stdout.contains("would_create_versions="));
    assert!(stdout.contains("bytes_to_write="));
}
