use crate::io::{run_with_policy, BlockingIoPolicy, CancellationFlag};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Summary: Resolves the default scheduled task XML path for Windows.
///
/// Inputs: none.
///
/// Outputs: the default XML path.
///
/// Side effects: Reads the roaming app data directory.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: CLI and GUI scheduled task installation flows.
///
/// Why this exists: centralize schtasks XML storage location.
pub fn default_task_xml_path() -> Result<PathBuf> {
    let mut path = dirs::data_dir().ok_or_else(|| anyhow::anyhow!("data dir not found"))?;
    path.push("backup_sync/backup_sync_task.xml");
    Ok(path)
}

/// Summary: Builds and optionally writes the schtasks XML definition.
///
/// Inputs: the destination path and daemon executable path.
///
/// Outputs: the XML content string.
///
/// Side effects: Creates directories and writes the XML when a destination is provided.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Windows service installation and export flows.
///
/// Why this exists: keep scheduled task definitions consistent across installs.
pub fn write_schtasks_xml(destination: &Path, exec: &Path) -> Result<String> {
    let io_policy = BlockingIoPolicy::bootstrap_defaults();
    let exec_str = exec.display().to_string();
    let contents = format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>true</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exec}</Command>
    </Exec>
  </Actions>
</Task>
"#,
        exec = exec_str
    );
    if let Some(parent) = destination.parent() {
        run_with_policy(
            "backup_core::service::windows_service::write_schtasks_xml create parent directory",
            &io_policy,
            CancellationFlag::none(),
            || {
                fs::create_dir_all(parent).with_context(|| {
                    format!("failed to create Windows task directory {:?}", parent)
                })
            },
        )?;
    }
    run_with_policy(
        "backup_core::service::windows_service::write_schtasks_xml write xml",
        &io_policy,
        CancellationFlag::none(),
        || {
            fs::write(destination, &contents)
                .with_context(|| format!("failed to write scheduled task XML to {:?}", destination))
        },
    )?;
    Ok(contents)
}
