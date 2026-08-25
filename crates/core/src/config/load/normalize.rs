use crate::config::model::{self, Config, Destination};

/// Older configs may omit newer fields (like `destinations`), but validation and runtime require them.
pub fn normalize_loaded_config(mut cfg: Config) -> Config {
    if cfg.destinations.is_empty() {
        cfg.destinations = vec![Destination {
            id: model::default_destination_id(),
            path: cfg.backup_root.clone(),
            label: Some("Primary".to_string()),
            max_backups_per_file: None,
            replicate_to: vec![],
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
