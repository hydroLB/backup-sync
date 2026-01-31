use anyhow::{Context, Result};
use std::{
    io::{self, Write},
    path::PathBuf,
};

/// Purpose: Prompts the user for a string input and trims the response.
///
/// Inputs: the prompt message.
/// Outputs: the trimmed user input string.
/// Ties to: interactive CLI flows such as init.
/// Side effects: Writes to stdout and reads from stdin.
/// Why: centralize prompt handling with consistent IO behavior.
pub fn prompt_string(msg: &str) -> Result<String> {
    print!("{} ", msg);
    io::stdout().flush().ok();
    let mut buf = String::new();
    io::stdin()
        .read_line(&mut buf)
        .context("cli::prompt_string failed to read input")?;
    Ok(buf.trim().to_string())
}

/// Purpose: Prompts the user for a yes or no response with a default.
///
/// Inputs: the prompt message and the default answer.
/// Outputs: the parsed boolean response.
/// Ties to: interactive CLI flows for confirmation prompts.
/// Side effects: Writes to stdout and reads from stdin.
/// Why: keep confirmation prompts consistent across commands.
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

/// Purpose: Prompts the user for a path, applying defaults and tilde expansion.
///
/// Inputs: the prompt message and an optional default string.
/// Outputs: a resolved `PathBuf`.
/// Ties to: interactive CLI flows for path inputs.
/// Side effects: Writes to stdout and reads from stdin.
/// Why: normalize path inputs before writing config.
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

/// Purpose: Expands a tilde prefixed path to the user home directory.
///
/// Inputs: the raw input string.
/// Outputs: the expanded string with home directory resolved when applicable.
/// Ties to: prompt path handling.
/// Side effects: Reads the user home directory.
/// Why: allow convenient user input for home relative paths.
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
