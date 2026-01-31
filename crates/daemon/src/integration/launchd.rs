use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Purpose: Resolves the default launchd plist path.
///
/// Inputs: none.
/// Outputs: the default plist path.
/// Ties to: service installation on macOS.
/// Side effects: Reads the user home directory.
/// Why: keep launchd manifest placement consistent.
pub fn default_plist_path() -> Result<PathBuf> {
    let mut path = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("home directory not found"))?;
    path.push("Library/LaunchAgents/com.backup_sync.daemon.plist");
    Ok(path)
}

/// Purpose: Builds and optionally writes the launchd plist manifest.
///
/// Inputs: the destination path, daemon executable path, and optional log path.
/// Outputs: the plist content string.
/// Ties to: macOS service installation and export flows.
/// Side effects: Creates directories and writes the plist when a destination is provided.
/// Why: keep launchd manifests consistent across installs.
pub fn write_plist(destination: &Path, exec: &Path, log_path: Option<&Path>) -> Result<String> {
    let exec_str = exec.display().to_string();
    let log_str = log_path
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "/tmp/backup_sync.log".to_string());
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
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create launchd directory {:?}", parent))?;
    }
    fs::write(destination, &contents)
        .with_context(|| format!("failed to write launchd plist to {:?}", destination))?;
    Ok(contents)
}
