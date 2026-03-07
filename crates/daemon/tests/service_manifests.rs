#[cfg(target_os = "macos")]
use backup_core::service::launchd;
#[cfg(target_os = "linux")]
use backup_core::service::systemd;
#[cfg(target_os = "windows")]
use backup_core::service::windows_service;
use std::path::PathBuf;

#[cfg(target_os = "macos")]
#[test]
/// Summary: Verify the launchd plist includes the expected executable path.
///
/// Inputs: None.
///
/// Outputs: Asserts on plist contents for launchd.
///
/// Side effects: Creates an in-memory plist string.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `backup_core::service::launchd::write_plist`.
///
/// Why this exists: Prevent regressions in macOS service manifest generation.
fn launchd_manifest_contains_exec() {
    let exec = PathBuf::from("/usr/local/bin/daemon");
    let plist = launchd::write_plist(PathBuf::new().as_path(), &exec, None)
        .expect("service_manifests::launchd_manifest_contains_exec failed to write plist");
    assert!(plist.contains("com.backup_sync.daemon"));
    assert!(plist.contains("/usr/local/bin/daemon"));
}

#[cfg(target_os = "linux")]
#[test]
/// Summary: Verify the systemd unit includes the expected executable path.
///
/// Inputs: None.
///
/// Outputs: Asserts on unit contents for systemd.
///
/// Side effects: Creates an in-memory unit string.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `backup_core::service::systemd::write_unit`.
///
/// Why this exists: Prevent regressions in Linux service manifest generation.
fn systemd_manifest_contains_exec() {
    let exec = PathBuf::from("/usr/local/bin/daemon");
    let unit = systemd::write_unit(PathBuf::new().as_path(), &exec, true, None)
        .expect("service_manifests::systemd_manifest_contains_exec failed to write unit");
    assert!(unit.contains("ExecStart=/usr/local/bin/daemon"));
    assert!(unit.contains("Description=Backup Sync Daemon"));
}

#[cfg(target_os = "windows")]
#[test]
/// Summary: Verify the scheduled task XML includes the expected executable path.
///
/// Inputs: None.
///
/// Outputs: Asserts on XML contents for Windows tasks.
///
/// Side effects: Creates an in-memory XML string.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `backup_core::service::windows_service::write_schtasks_xml`.
///
/// Why this exists: Prevent regressions in Windows service manifest generation.
fn windows_task_contains_exec() {
    let exec = PathBuf::from("C:/Program Files/daemon.exe");
    let xml = windows_service::write_schtasks_xml(PathBuf::new().as_path(), &exec)
        .expect("service_manifests::windows_task_contains_exec failed to write task XML");
    assert!(xml.contains("daemon.exe"));
    assert!(xml.contains("<Task"));
}
