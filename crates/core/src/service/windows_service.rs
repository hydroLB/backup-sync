use crate::io::{run_with_policy, BlockingIoPolicy, CancellationFlag};
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Centralize schtasks XML storage location.
pub fn default_task_xml_path() -> Result<PathBuf> {
    let mut path = dirs::data_dir().ok_or_else(|| anyhow::anyhow!("data dir not found"))?;
    path.push("backup_sync/backup_sync_task.xml");
    Ok(path)
}

/// Keep scheduled task definitions consistent across installs.
pub fn write_schtasks_xml(destination: &Path, exec: &Path) -> Result<String> {
    let io_policy = BlockingIoPolicy::bootstrap_defaults();
    let exec_str = escape_xml_text(&exec.display().to_string(), "executable path")?;
    let contents = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
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
    if destination.as_os_str().is_empty() {
        return Ok(contents);
    }
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

fn escape_xml_text(value: &str, field: &str) -> Result<String> {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if !is_xml_10_character(character) {
            bail!(
                "{field} contains a character that XML 1.0 cannot represent: U+{:04X}",
                character as u32
            );
        }

        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(character),
        }
    }
    Ok(escaped)
}

fn is_xml_10_character(character: char) -> bool {
    matches!(character, '\u{9}' | '\u{A}' | '\u{D}')
        || matches!(character as u32, 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10FFFF)
}
