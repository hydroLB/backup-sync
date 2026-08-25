mod support;

#[test]
/// Protect user-facing command discoverability from accidental regressions.
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
    assert!(stdout.contains("Usage: backup-sync <COMMAND>"));
    assert!(stdout.contains("status           Show the latest backup state"));
    assert!(stdout.contains("doctor           Diagnose configuration"));
    assert!(stdout.contains("run-once         Run one backup cycle"));
    assert!(stdout.contains("verify           Verify the latest stored copies"));
    assert!(stdout.contains("install-service  Generate or install"));
    assert!(
        !stdout.contains("Summary:"),
        "cli_contract::cli_help_contract_lists_supported_commands leaked internal docs: {stdout}"
    );
}

#[test]
/// Keep scripting and operator expectations stable for status output.
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
/// Prevent accidental contract drift in dry-run operational reporting.
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
