use crate::config::model::{self, Config, Destination};

/// Summary: Normalizes a loaded config to keep backward compatibility across schema versions.
///
/// Inputs: A parsed `Config` value (possibly from an older on-disk schema).
/// Outputs: A `Config` with required derived fields populated.
/// Side effects: None.
/// Error handling: None (best-effort normalization only; never fails).
/// Ties to other methods: Called by `config::load::file::{load_config, load_from_path}`.
/// Why this exists: Older configs may omit newer fields (like `destinations`), but validation and runtime require them.
pub fn normalize_loaded_config(mut cfg: Config) -> Config {
    if cfg.destinations.is_empty() {
        cfg.destinations = vec![Destination {
            id: model::default_destination_id(),
            path: cfg.backup_root.clone(),
            label: Some("Primary".to_string()),
            max_backups_per_file: None,
        }];
    }

    let default_dest_id = model::default_destination_id();
    for w in &mut cfg.watched {
        if w.destination_id.trim().is_empty() {
            w.destination_id = default_dest_id.clone();
        }
    }

    cfg
}
