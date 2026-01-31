#[cfg(target_os = "macos")]
use daemon::integration::launchd;
#[cfg(target_os = "linux")]
use daemon::integration::systemd;
#[cfg(target_os = "windows")]
use daemon::integration::windows_service;
use std::path::PathBuf;

#[cfg(target_os = "macos")]
#[test]
/// Purpose: Verify the launchd plist includes the expected executable path.
///
/// Inputs: None.
/// Outputs: Asserts on plist contents for launchd.
/// Ties to: `daemon::integration::launchd::write_plist`.
/// Side effects: Creates an in-memory plist string.
/// Why: Prevent regressions in macOS service manifest generation.
fn launchd_manifest_contains_exec() {
    let exec = PathBuf::from("/usr/local/bin/daemon");
    let plist = launchd::write_plist(PathBuf::new().as_path(), &exec, None)
        .expect("service_manifests::launchd_manifest_contains_exec failed to write plist");
    assert!(plist.contains("com.backup_sync.daemon"));
    assert!(plist.contains("/usr/local/bin/daemon"));
}

#[cfg(target_os = "linux")]
#[test]
/// Purpose: Verify the systemd unit includes the expected executable path.
///
/// Inputs: None.
/// Outputs: Asserts on unit contents for systemd.
/// Ties to: `daemon::integration::systemd::write_unit`.
/// Side effects: Creates an in-memory unit string.
/// Why: Prevent regressions in Linux service manifest generation.
fn systemd_manifest_contains_exec() {
    let exec = PathBuf::from("/usr/local/bin/daemon");
    let unit = systemd::write_unit(PathBuf::new().as_path(), &exec, true, None)
        .expect("service_manifests::systemd_manifest_contains_exec failed to write unit");
    assert!(unit.contains("ExecStart=/usr/local/bin/daemon"));
    assert!(unit.contains("Description=Backup Sync Daemon"));
}

#[cfg(target_os = "windows")]
#[test]
/// Purpose: Verify the scheduled task XML includes the expected executable path.
///
/// Inputs: None.
/// Outputs: Asserts on XML contents for Windows tasks.
/// Ties to: `daemon::integration::windows_service::write_schtasks_xml`.
/// Side effects: Creates an in-memory XML string.
/// Why: Prevent regressions in Windows service manifest generation.
fn windows_task_contains_exec() {
    let exec = PathBuf::from("C:/Program Files/daemon.exe");
    let xml = windows_service::write_schtasks_xml(PathBuf::new().as_path(), &exec)
        .expect("service_manifests::windows_task_contains_exec failed to write task XML");
    assert!(xml.contains("daemon.exe"));
    assert!(xml.contains("<Task"));
}
