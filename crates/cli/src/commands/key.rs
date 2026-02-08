use anyhow::{Context, Result};
use backup_core::encryption::keyfile;
use backup_core::platform::paths;
use std::path::PathBuf;

/// Purpose: Create a new encryption key file for at-rest blob encryption.
///
/// Inputs: Optional key path override and a force overwrite flag.
/// Outputs: `Ok(())` after writing the key file and printing recovery guidance.
/// Ties to: encryption config (`[encryption]`) and versioned blob encryption.
/// Side effects: Writes a key file to disk with restrictive permissions where supported.
/// Why: encrypted backups are only recoverable with the same key; key creation must be explicit and user-controlled.
pub fn keygen(path: Option<PathBuf>, force: bool) -> Result<()> {
    let out_path = match path {
        Some(p) => p,
        None => paths::encryption_key_file_path()
            .context("cli::keygen failed to resolve default encryption key path")?,
    };
    let info = keyfile::create_key_file(&out_path, force)
        .context("cli::keygen failed to create encryption key file")?;

    println!("Encryption key created at: {:?}", out_path);
    println!("key_id: {}", info.key_id);
    println!();
    println!("Recovery notes:");
    println!(
        "- Keep a secure copy of the key file. Losing it makes encrypted backups unrecoverable."
    );
    println!("- To restore on a new machine, copy the key file and set the same encryption.key_id in config.");
    println!();
    println!("Config snippet:");
    println!("[encryption]");
    println!("enabled = true");
    println!("key_id = \"{}\"", info.key_id);
    println!(
        "# key_path = {:?}  # optional when using the default path",
        out_path
    );
    Ok(())
}
