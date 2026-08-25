use crate::io::{run_with_policy, BlockingIoPolicy, CancellationFlag};
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Keep launchd manifest placement consistent.
pub fn default_plist_path() -> Result<PathBuf> {
    let mut path = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("home directory not found"))?;
    path.push("Library/LaunchAgents/com.backup_sync.daemon.plist");
    Ok(path)
}

/// Keep launchd manifests consistent across installs.
pub fn write_plist(destination: &Path, exec: &Path, log_path: Option<&Path>) -> Result<String> {
    let io_policy = BlockingIoPolicy::bootstrap_defaults();
    let exec_str = escape_xml_text(&exec.display().to_string(), "executable path")?;
    let log_str = escape_xml_text(
        &log_path
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "/tmp/backup_sync.log".to_string()),
        "log path",
    )?;
    let contents = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>com.backup_sync.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exec}</string>
    </array>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key><true/>
    <key>StandardOutPath</key><string>{log}</string>
    <key>StandardErrorPath</key><string>{log}</string>
</dict>
</plist>
"#,
        exec = exec_str,
        log = log_str
    );

    if destination.as_os_str().is_empty() {
        return Ok(contents);
    }
    if let Some(parent) = destination.parent() {
        run_with_policy(
            "backup_core::service::launchd::write_plist create parent directory",
            &io_policy,
            CancellationFlag::none(),
            || {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create launchd directory {:?}", parent))
            },
        )?;
    }
    run_with_policy(
        "backup_core::service::launchd::write_plist write plist",
        &io_policy,
        CancellationFlag::none(),
        || {
            fs::write(destination, &contents)
                .with_context(|| format!("failed to write launchd plist to {:?}", destination))
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
