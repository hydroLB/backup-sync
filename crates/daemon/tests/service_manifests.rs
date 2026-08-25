use backup_core::service::{launchd, systemd, windows_service};
use std::path::PathBuf;

#[test]
/// Prevent regressions in macOS service manifest generation.
fn launchd_manifest_contains_exec() {
    let exec = PathBuf::from("/usr/local/bin/daemon");
    let plist = launchd::write_plist(PathBuf::new().as_path(), &exec, None)
        .expect("service_manifests::launchd_manifest_contains_exec failed to write plist");
    assert!(plist.contains("com.backup_sync.daemon"));
    assert!(plist.contains("/usr/local/bin/daemon"));
}

#[test]
/// Prevent regressions in Linux service manifest generation.
fn systemd_manifest_contains_exec() {
    let exec = PathBuf::from("/usr/local/bin/daemon");
    let unit = systemd::write_unit(PathBuf::new().as_path(), &exec, true, None)
        .expect("service_manifests::systemd_manifest_contains_exec failed to write unit");
    assert!(unit.contains("ExecStart=\"/usr/local/bin/daemon\""));
    assert!(unit.contains("Description=Backup Sync Daemon"));
}

#[test]
/// Prevent regressions in Windows service manifest generation.
fn windows_task_contains_exec() {
    let exec = PathBuf::from("C:/Program Files/daemon.exe");
    let xml = windows_service::write_schtasks_xml(PathBuf::new().as_path(), &exec)
        .expect("service_manifests::windows_task_contains_exec failed to write task XML");
    assert!(xml.contains("daemon.exe"));
    assert!(xml.contains("<Task"));
}

#[test]
fn xml_manifests_escape_special_paths_and_declare_utf8() {
    let exec = PathBuf::from(r#"/opt/Backup & Sync/<daemon> "quoted" 'single'"#);
    let log = PathBuf::from(r#"/tmp/Backup & Sync/<daemon> "quoted" 'single'.log"#);

    let plist = launchd::write_plist(PathBuf::new().as_path(), &exec, Some(&log))
        .expect("launchd should accept XML-safe special paths");
    assert!(plist.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(plist
        .contains("/opt/Backup &amp; Sync/&lt;daemon&gt; &quot;quoted&quot; &apos;single&apos;"));
    assert!(plist.contains(
        "/tmp/Backup &amp; Sync/&lt;daemon&gt; &quot;quoted&quot; &apos;single&apos;.log"
    ));

    let task = windows_service::write_schtasks_xml(PathBuf::new().as_path(), &exec)
        .expect("Windows task XML should support generation without writing");
    assert!(task.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(task
        .contains("/opt/Backup &amp; Sync/&lt;daemon&gt; &quot;quoted&quot; &apos;single&apos;"));
}

#[test]
fn xml_manifests_reject_disallowed_control_characters() {
    let exec = PathBuf::from("/opt/daemon\u{1}.exe");
    assert!(launchd::write_plist(PathBuf::new().as_path(), &exec, None).is_err());
    assert!(windows_service::write_schtasks_xml(PathBuf::new().as_path(), &exec).is_err());

    let log = PathBuf::from("/tmp/daemon\u{b}.log");
    assert!(launchd::write_plist(
        PathBuf::new().as_path(),
        PathBuf::from("/bin/daemon").as_path(),
        Some(&log)
    )
    .is_err());
}

#[test]
fn systemd_manifest_quotes_and_escapes_special_paths() {
    let exec = PathBuf::from("/opt/Backup Sync/daemon \"quoted\" $cash %instance\\tool\t");
    let log = PathBuf::from("/tmp/Backup Sync/%i \"daemon\" $cash.log");
    let unit = systemd::write_unit(PathBuf::new().as_path(), &exec, true, Some(&log))
        .expect("systemd should quote special paths");

    assert!(unit
        .contains(r#"ExecStart="/opt/Backup Sync/daemon \"quoted\" $$cash %%instance\\tool\t""#));
    assert!(unit.contains(r#"StandardOutput="append:/tmp/Backup Sync/%%i \"daemon\" $cash.log""#));
    assert!(unit.contains(r#"StandardError="append:/tmp/Backup Sync/%%i \"daemon\" $cash.log""#));
}

#[test]
fn systemd_manifest_rejects_newlines_in_paths() {
    let exec = PathBuf::from("/opt/daemon\nExecStart=/tmp/other");
    assert!(systemd::write_unit(PathBuf::new().as_path(), &exec, true, None).is_err());

    let log = PathBuf::from("/tmp/daemon\r\n[Install].log");
    assert!(systemd::write_unit(
        PathBuf::new().as_path(),
        PathBuf::from("/bin/daemon").as_path(),
        true,
        Some(&log),
    )
    .is_err());
}
