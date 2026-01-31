use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Purpose: Resolves the default scheduled task XML path for Windows.
///
/// Inputs: none.
/// Outputs: the default XML path.
/// Ties to: CLI and GUI scheduled task installation flows.
/// Side effects: Reads the roaming app data directory.
/// Why: centralize schtasks XML storage location.
pub fn default_task_xml_path() -> Result<PathBuf> {
    let mut path = dirs::data_dir().ok_or_else(|| anyhow::anyhow!("data dir not found"))?;
    path.push("backup_sync/backup_sync_task.xml");
    Ok(path)
}

/// Purpose: Builds and optionally writes the schtasks XML definition.
///
/// Inputs: the destination path and daemon executable path.
/// Outputs: the XML content string.
/// Ties to: Windows service installation and export flows.
/// Side effects: Creates directories and writes the XML when a destination is provided.
/// Why: keep scheduled task definitions consistent across installs.
pub fn write_schtasks_xml(destination: &Path, exec: &Path) -> Result<String> {
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
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create Windows task directory {:?}", parent))?;
    }
    fs::write(destination, &contents)
        .with_context(|| format!("failed to write scheduled task XML to {:?}", destination))?;
    Ok(contents)
}
