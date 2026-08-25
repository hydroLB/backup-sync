use crate::io::{run_with_policy, BlockingIoPolicy, CancellationFlag};
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Centralize systemd path decisions for consistent installs.
pub fn default_unit_path(user: bool) -> Result<PathBuf> {
    if user {
        let mut path = dirs::config_dir().ok_or_else(|| anyhow::anyhow!("config dir not found"))?;
        path.push("systemd/user/backup-sync.service");
        Ok(path)
    } else {
        Ok(PathBuf::from("/etc/systemd/system/backup-sync.service"))
    }
}

/// Keep service unit formatting consistent across installation paths.
pub fn write_unit(
    destination: &Path,
    exec: &Path,
    user: bool,
    log_path: Option<&Path>,
) -> Result<String> {
    let io_policy = BlockingIoPolicy::bootstrap_defaults();
    let exec_str = quote_unit_value(&exec.display().to_string(), "executable path", true)?;
    let log_str = quote_unit_value(
        &format!(
            "append:{}",
            log_path
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "/var/log/backup_sync.log".to_string())
        ),
        "log path",
        false,
    )?;
    let after = if user {
        "default.target"
    } else {
        "network.target"
    };
    let contents = format!(
        "[Unit]
Description=Backup Sync Daemon
After={after}

[Service]
Type=simple
ExecStart={exec}
Restart=on-failure
StandardOutput={log}
StandardError={log}

[Install]
WantedBy=default.target
",
        exec = exec_str,
        log = log_str,
        after = after
    );

    if destination.as_os_str().is_empty() {
        return Ok(contents);
    }
    if let Some(parent) = destination.parent() {
        run_with_policy(
            "backup_core::service::systemd::write_unit create parent directory",
            &io_policy,
            CancellationFlag::none(),
            || {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create systemd directory {:?}", parent))
            },
        )?;
    }
    run_with_policy(
        "backup_core::service::systemd::write_unit write unit",
        &io_policy,
        CancellationFlag::none(),
        || {
            fs::write(destination, &contents)
                .with_context(|| format!("failed to write systemd unit to {:?}", destination))
        },
    )?;
    Ok(contents)
}

fn quote_unit_value(value: &str, field: &str, escape_dollar: bool) -> Result<String> {
    if value.contains(['\n', '\r']) {
        bail!("{field} must not contain newline characters");
    }

    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for character in value.chars() {
        match character {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '$' if escape_dollar => quoted.push_str("$$"),
            '%' => quoted.push_str("%%"),
            '\t' => quoted.push_str("\\t"),
            '\u{07}' => quoted.push_str("\\a"),
            '\u{08}' => quoted.push_str("\\b"),
            '\u{0B}' => quoted.push_str("\\v"),
            '\u{0C}' => quoted.push_str("\\f"),
            character if character.is_control() => {
                use std::fmt::Write;
                write!(quoted, "\\u{:04x}", character as u32)
                    .expect("writing to a String cannot fail");
            }
            _ => quoted.push(character),
        }
    }
    quoted.push('"');
    Ok(quoted)
}
