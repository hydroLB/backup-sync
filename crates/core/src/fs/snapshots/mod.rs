use crate::logging::redact_path;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Purpose: Records metadata about a snapshot used to produce a consistent view of a source path.
///
/// Inputs: Produced by platform-specific snapshot providers.
/// Outputs: Serialized into manifests and scan reports.
/// Ties to: versioned backup scanning and blob writes for crash-consistent commits.
/// Side effects: None.
/// Why: make it explicit whether a version was derived from a live filesystem view or a snapshot view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSnapshotInfo {
    pub provider: String,
    #[serde(default)]
    pub id: Option<String>,
    pub created_at_unix: i64,
}

/// Purpose: Represents a source path view that should be used for scanning and blob reads.
///
/// Inputs: Original watched path and runtime snapshot settings.
/// Outputs: Provides a scan path and optional snapshot metadata plus cleanup guard.
/// Ties to: versioned store `scan_snapshot` and blob writing.
/// Side effects: May create and mount snapshots (best-effort).
/// Why: scanning and reading blobs must use the same consistent view to avoid mismatched manifests.
#[derive(Debug)]
pub struct SourcePathView {
    scan_path: PathBuf,
    snapshot: Option<SourceSnapshotInfo>,
    snapshot_error: Option<String>,
    _guard: Option<SnapshotGuard>,
}

impl SourcePathView {
    pub fn scan_path(&self) -> &Path {
        &self.scan_path
    }

    pub fn snapshot(&self) -> Option<&SourceSnapshotInfo> {
        self.snapshot.as_ref()
    }

    pub fn snapshot_error(&self) -> Option<&str> {
        self.snapshot_error.as_deref()
    }
}

#[derive(Debug)]
#[allow(dead_code)]
enum SnapshotGuard {
    #[cfg(target_os = "macos")]
    MacosMounted(MacosMountedSnapshot),
    #[cfg(target_os = "windows")]
    WindowsVss(WindowsVssSnapshot),
}

/// Purpose: Prepare a best-effort snapshot-backed view of a watched path.
///
/// Inputs: The original watched path, a boolean enable flag, and a timeout in seconds for any OS
/// command invocations.
/// Outputs: A `SourcePathView` that points either at a snapshot-mounted path or at the original path.
/// Side effects: May create and mount a snapshot, and will attempt cleanup on drop when supported.
/// Error handling: Never fails the backup cycle when snapshot operations fail; returns the original
/// path with an explanatory error string.
/// Ties to other methods: Intended to wrap both scanning and blob reads in the versioned store.
/// Why this exists: A changing or glitching source can yield inconsistent manifests unless reads are
/// taken from a stable snapshot view.
pub fn prepare_source_view(
    path: &Path,
    enabled: bool,
    timeout_seconds: u64,
) -> Result<SourcePathView> {
    if !enabled {
        return Ok(SourcePathView {
            scan_path: path.to_path_buf(),
            snapshot: None,
            snapshot_error: None,
            _guard: None,
        });
    }

    let timeout_seconds = timeout_seconds.max(1);
    let canonical = match path.canonicalize() {
        Ok(p) => p,
        Err(_) => path.to_path_buf(),
    };

    #[cfg(target_os = "macos")]
    {
        match try_macos_tmutil_apfs_snapshot_view(&canonical, Duration::from_secs(timeout_seconds))
        {
            Ok(Some(prepared)) => {
                return Ok(SourcePathView {
                    scan_path: prepared.scan_path,
                    snapshot: Some(prepared.info),
                    snapshot_error: None,
                    _guard: Some(SnapshotGuard::MacosMounted(prepared.guard)),
                });
            }
            Ok(None) => {
                return Ok(SourcePathView {
                    scan_path: path.to_path_buf(),
                    snapshot: None,
                    snapshot_error: Some("snapshot provider unavailable for this path".to_string()),
                    _guard: None,
                });
            }
            Err(e) => {
                return Ok(SourcePathView {
                    scan_path: path.to_path_buf(),
                    snapshot: None,
                    snapshot_error: Some(format!("{e:#}")),
                    _guard: None,
                });
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        match try_windows_vss_snapshot_view(&canonical, Duration::from_secs(timeout_seconds)) {
            Ok(Some(prepared)) => {
                return Ok(SourcePathView {
                    scan_path: prepared.scan_path,
                    snapshot: Some(prepared.info),
                    snapshot_error: None,
                    _guard: Some(SnapshotGuard::WindowsVss(prepared.guard)),
                });
            }
            Ok(None) => {
                return Ok(SourcePathView {
                    scan_path: path.to_path_buf(),
                    snapshot: None,
                    snapshot_error: Some(
                        "VSS snapshot provider unavailable for this path".to_string(),
                    ),
                    _guard: None,
                });
            }
            Err(e) => {
                return Ok(SourcePathView {
                    scan_path: path.to_path_buf(),
                    snapshot: None,
                    snapshot_error: Some(format!("{e:#}")),
                    _guard: None,
                });
            }
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Ok(SourcePathView {
            scan_path: path.to_path_buf(),
            snapshot: None,
            snapshot_error: Some("source snapshots are not supported on this platform".to_string()),
            _guard: None,
        })
    }
}

fn run_command_with_timeout(mut cmd: Command, timeout: Duration) -> Result<Output> {
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .context("fs::snapshots::run_command_with_timeout failed to spawn command")?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .context("fs::snapshots::run_command_with_timeout failed to collect output");
            }
            Ok(None) => {}
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "fs::snapshots::run_command_with_timeout failed to poll child: {e}"
                ));
            }
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!(
                "fs::snapshots::run_command_with_timeout timed out after {}s",
                timeout.as_secs().max(1)
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(target_os = "macos")]
#[derive(Debug)]
struct MacosMountedSnapshot {
    mount_dir: tempfile::TempDir,
    timeout: Duration,
}

#[cfg(target_os = "macos")]
impl Drop for MacosMountedSnapshot {
    fn drop(&mut self) {
        let mount = self.mount_dir.path().to_path_buf();
        let timeout = self.timeout;
        let mut cmd = Command::new("umount");
        cmd.arg(&mount);
        let res = run_command_with_timeout(cmd, timeout);
        if let Err(e) = res {
            tracing::warn!(
                mountpoint = %redact_path(&mount),
                error = %e,
                "fs::snapshots failed to unmount macOS snapshot mountpoint"
            );
        }
    }
}

#[cfg(target_os = "macos")]
#[derive(Debug)]
struct PreparedSnapshot {
    scan_path: PathBuf,
    info: SourceSnapshotInfo,
    guard: MacosMountedSnapshot,
}

#[cfg(any(target_os = "macos", test))]
fn parse_df_posix_line(line: &str) -> Result<(String, PathBuf)> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 6 {
        anyhow::bail!(
            "fs::snapshots::parse_df_posix_line expected >=6 columns, got {} in {:?}",
            parts.len(),
            line
        );
    }
    let device = parts[0].to_string();
    let mountpoint = parts[5..].join(" ");
    Ok((device, PathBuf::from(mountpoint)))
}

#[cfg(target_os = "macos")]
fn df_mount_info(path: &Path, timeout: Duration) -> Result<(String, PathBuf)> {
    let mut cmd = Command::new("df");
    cmd.arg("-P").arg(path);
    let output = run_command_with_timeout(cmd, timeout)
        .with_context(|| format!("fs::snapshots::df_mount_info failed for {:?}", path))?;
    if !output.status.success() {
        anyhow::bail!(
            "fs::snapshots::df_mount_info df exited non-zero (status={})",
            output.status
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .nth(1)
        .context("fs::snapshots::df_mount_info missing df output line")?;
    parse_df_posix_line(line)
}

#[cfg(any(target_os = "macos", test))]
fn parse_tmutil_listlocalsnapshots_output(stdout: &str) -> Option<String> {
    let mut best: Option<String> = None;
    for line in stdout.lines() {
        let snap = line.trim();
        if snap.starts_with("com.apple.TimeMachine.") {
            match &best {
                None => best = Some(snap.to_string()),
                Some(cur) => {
                    if snap > cur.as_str() {
                        best = Some(snap.to_string());
                    }
                }
            }
        }
    }
    best
}

#[cfg(target_os = "macos")]
fn try_macos_tmutil_apfs_snapshot_view(
    path: &Path,
    timeout: Duration,
) -> Result<Option<PreparedSnapshot>> {
    let (device, mountpoint) = df_mount_info(path, timeout)?;

    let mut create_cmd = Command::new("tmutil");
    create_cmd.arg("localsnapshot");
    let create = run_command_with_timeout(create_cmd, timeout).context(
        "fs::snapshots::try_macos_tmutil_apfs_snapshot_view tmutil localsnapshot failed",
    )?;
    if !create.status.success() {
        anyhow::bail!(
            "fs::snapshots::try_macos_tmutil_apfs_snapshot_view tmutil localsnapshot exited non-zero (status={})",
            create.status
        );
    }

    let mut list_cmd = Command::new("tmutil");
    list_cmd.arg("listlocalsnapshots").arg(&mountpoint);
    let list = run_command_with_timeout(list_cmd, timeout).context(
        "fs::snapshots::try_macos_tmutil_apfs_snapshot_view tmutil listlocalsnapshots failed",
    )?;
    if !list.status.success() {
        anyhow::bail!(
            "fs::snapshots::try_macos_tmutil_apfs_snapshot_view tmutil listlocalsnapshots exited non-zero (status={})",
            list.status
        );
    }
    let stdout = String::from_utf8_lossy(&list.stdout);
    let snapshot_name = match parse_tmutil_listlocalsnapshots_output(&stdout) {
        Some(s) => s,
        None => return Ok(None),
    };

    let mount_dir =
        tempfile::TempDir::new().context("fs::snapshots failed to create mount tempdir")?;
    let mountpoint_path = mount_dir.path().to_path_buf();

    let mut mount_cmd = Command::new("mount_apfs");
    mount_cmd
        .arg("-o")
        .arg("ro,nobrowse")
        .arg("-s")
        .arg(&snapshot_name)
        .arg(&device)
        .arg(&mountpoint_path);
    let mount = run_command_with_timeout(mount_cmd, timeout).with_context(|| {
        format!(
            "fs::snapshots::try_macos_tmutil_apfs_snapshot_view failed to mount snapshot {} on {}",
            snapshot_name, device
        )
    })?;
    if !mount.status.success() {
        anyhow::bail!(
            "fs::snapshots::try_macos_tmutil_apfs_snapshot_view mount_apfs exited non-zero (status={})",
            mount.status
        );
    }

    let rel = path
        .strip_prefix(&mountpoint)
        .with_context(|| {
            format!(
                "fs::snapshots::try_macos_tmutil_apfs_snapshot_view could not map {:?} under mountpoint {:?}",
                path, mountpoint
            )
        })?;
    let scan_path = mountpoint_path.join(rel);

    let guard = MacosMountedSnapshot { mount_dir, timeout };

    Ok(Some(PreparedSnapshot {
        scan_path,
        info: SourceSnapshotInfo {
            provider: "macos_tmutil_apfs".to_string(),
            id: Some(snapshot_name),
            created_at_unix: chrono::Utc::now().timestamp(),
        },
        guard,
    }))
}

#[cfg(target_os = "windows")]
#[derive(Debug)]
struct WindowsVssSnapshot {
    shadow_id: String,
    timeout: Duration,
}

#[cfg(target_os = "windows")]
impl Drop for WindowsVssSnapshot {
    fn drop(&mut self) {
        let shadow_id = self.shadow_id.clone();
        let timeout = self.timeout;
        let script = format!(
            "$id='{shadow_id}'; $s=Get-CimInstance -ClassName Win32_ShadowCopy | Where-Object {{$_.ID -eq $id}}; if ($s) {{ $s | Remove-CimInstance }}",
        );
        let mut cmd = Command::new("powershell.exe");
        cmd.arg("-NoProfile")
            .arg("-NonInteractive")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-Command")
            .arg(script);
        let res = run_command_with_timeout(cmd, timeout);
        if let Err(e) = res {
            tracing::warn!(
                shadow_id = %shadow_id,
                error = %e,
                "fs::snapshots failed to delete VSS shadow copy"
            );
        }
    }
}

#[cfg(target_os = "windows")]
#[derive(Debug)]
struct PreparedWindowsSnapshot {
    scan_path: PathBuf,
    info: SourceSnapshotInfo,
    guard: WindowsVssSnapshot,
}

#[cfg(target_os = "windows")]
fn try_windows_vss_snapshot_view(
    path: &Path,
    timeout: Duration,
) -> Result<Option<PreparedWindowsSnapshot>> {
    let path_str = path.to_string_lossy().to_string();
    let volume = match extract_windows_volume_root(&path_str) {
        Some(v) => v,
        None => return Ok(None),
    };
    let rel = path_str
        .strip_prefix(&volume)
        .unwrap_or(path_str.as_str())
        .trim_start_matches(['\\', '/']);

    let script = format!(
        "$vol='{volume}'; $r=Invoke-CimMethod -ClassName Win32_ShadowCopy -MethodName Create -Arguments @{{Volume=$vol; Context='ClientAccessible'}}; $id=$r.ShadowID; $dev=(Get-CimInstance -ClassName Win32_ShadowCopy | Where-Object {{$_.ID -eq $id}} | Select-Object -First 1 -ExpandProperty DeviceObject); Write-Output ($id + '|' + $dev);",
    );
    let mut cmd = Command::new("powershell.exe");
    cmd.arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-Command")
        .arg(script);
    let output = run_command_with_timeout(cmd, timeout)
        .context("fs::snapshots::try_windows_vss_snapshot_view failed to create VSS snapshot")?;
    if !output.status.success() {
        anyhow::bail!(
            "fs::snapshots::try_windows_vss_snapshot_view powershell exited non-zero (status={})",
            output.status
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let (shadow_id, device) = match parse_windows_vss_output(&stdout) {
        Some(v) => v,
        None => anyhow::bail!(
            "fs::snapshots::try_windows_vss_snapshot_view could not parse VSS output: {:?}",
            stdout
        ),
    };

    let scan_path = if rel.is_empty() {
        PathBuf::from(format!("{device}\\"))
    } else {
        PathBuf::from(format!("{device}\\{rel}"))
    };

    Ok(Some(PreparedWindowsSnapshot {
        scan_path,
        info: SourceSnapshotInfo {
            provider: "windows_vss".to_string(),
            id: Some(shadow_id.clone()),
            created_at_unix: chrono::Utc::now().timestamp(),
        },
        guard: WindowsVssSnapshot { shadow_id, timeout },
    }))
}

#[cfg(any(target_os = "windows", test))]
fn extract_windows_volume_root(path: &str) -> Option<String> {
    if path.len() < 3 {
        return None;
    }
    let bytes = path.as_bytes();
    let c0 = bytes[0] as char;
    if !c0.is_ascii_alphabetic() || bytes[1] != b':' {
        return None;
    }
    Some(format!("{}:\\", c0.to_ascii_uppercase()))
}

#[cfg(any(target_os = "windows", test))]
fn parse_windows_vss_output(stdout: &str) -> Option<(String, String)> {
    for line in stdout.lines() {
        let l = line.trim();
        if l.is_empty() || !l.contains('|') {
            continue;
        }
        let mut parts = l.splitn(2, '|');
        let id = parts.next()?.trim().to_string();
        let dev = parts.next()?.trim().to_string();
        if id.is_empty() || dev.is_empty() {
            continue;
        }
        return Some((id, dev));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tmutil_snapshot_name() {
        let stdout = r#"
Snapshots for disk /:
com.apple.TimeMachine.2026-02-04-121314.local
com.apple.TimeMachine.2026-02-04-131415.local
"#;
        let best = parse_tmutil_listlocalsnapshots_output(stdout);
        assert_eq!(
            best.as_deref(),
            Some("com.apple.TimeMachine.2026-02-04-131415.local")
        );
    }

    #[test]
    fn parses_df_posix_line_with_spaces() {
        let line = "/dev/disk4s2 1953523712 123 456 12% /Volumes/External Drive";
        let (dev, mount) = parse_df_posix_line(line).unwrap();
        assert_eq!(dev, "/dev/disk4s2");
        assert_eq!(mount.to_string_lossy(), "/Volumes/External Drive");
    }

    #[test]
    fn windows_vss_parser_accepts_id_and_device() {
        let stdout = "{ID}|\\\\?\\GLOBALROOT\\Device\\HarddiskVolumeShadowCopy7\r\n";
        let parsed = parse_windows_vss_output(stdout);
        assert!(parsed.is_some());
    }

    #[test]
    fn windows_volume_root_extractor_parses_drive_letter() {
        let vol = extract_windows_volume_root("c:\\Users\\me\\file.txt");
        assert_eq!(vol.as_deref(), Some("C:\\"));
    }
}
