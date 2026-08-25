use anyhow::{Context, Result};
use backup_core::logging::cid;
use std::{
    io::{self, Write},
    path::PathBuf,
};
use tracing::warn;

/// Centralize prompt handling with consistent IO behavior.
pub fn prompt_string(msg: &str) -> Result<String> {
    print!("{} ", msg);
    if let Err(error) = io::stdout().flush() {
        let correlation_id = cid("cli-prompt");
        warn!(
            cid = %correlation_id,
            action = "flush_stdout_failed",
            error = %error,
            "cli::prompt_string failed to flush stdout before prompt; continuing"
        );
    }
    let mut buf = String::new();
    io::stdin()
        .read_line(&mut buf)
        .context("cli::prompt_string failed to read input")?;
    Ok(buf.trim().to_string())
}

/// Keep confirmation prompts consistent across commands.
pub fn prompt_yes(msg: &str, default_yes: bool) -> Result<bool> {
    let default_hint = if default_yes { "[Y/n]" } else { "[y/N]" };
    let input = prompt_string(&format!("{} {}", msg, default_hint))
        .context("cli::prompt_yes failed to read confirmation input")?;
    if input.is_empty() {
        return Ok(default_yes);
    }
    let first = input.chars().next().unwrap_or('n');
    Ok(matches!(first, 'y' | 'Y'))
}

/// Normalize path inputs before writing config.
pub fn prompt_path(msg: &str, default: Option<&str>) -> Result<PathBuf> {
    let input = prompt_string(msg).context("cli::prompt_path failed to read path input")?;
    let raw = if input.is_empty() {
        default.unwrap_or("").to_string()
    } else {
        input
    };
    if raw.is_empty() {
        anyhow::bail!("cli::prompt_path path cannot be empty");
    }
    Ok(PathBuf::from(expand_tilde(&raw)))
}

/// Allow convenient user input for home relative paths.
pub fn expand_tilde(input: &str) -> String {
    if let Some(stripped) = input.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped).to_string_lossy().to_string();
        }
    } else if input == "~" {
        if let Some(home) = dirs::home_dir() {
            return home.to_string_lossy().to_string();
        }
    }
    input.to_string()
}
