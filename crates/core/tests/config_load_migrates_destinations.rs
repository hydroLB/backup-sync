use backup_core::{load_from_path, validate};
use std::fs;

#[test]
fn load_from_path_migrates_missing_destinations_and_validates() {
    let temp = tempfile::tempdir().expect("tempdir");
    let watched = temp.path().join("watched");
    fs::create_dir_all(&watched).expect("create watched dir");

    let cfg_path = temp.path().join("config.toml");
    let backup_root = temp.path().join("backups");

    let toml = format!(
        r#"
backup_root = "{backup_root}"
interval_seconds = 60
max_backups_per_file = 3

[[watched]]
path = "{watched}"
kind = "Directory"
enabled = true
"#,
        backup_root = backup_root.display(),
        watched = watched.display()
    );
    fs::write(&cfg_path, toml).expect("write config");

    let cfg = load_from_path(&cfg_path).expect("load_from_path");
    assert!(
        !cfg.destinations.is_empty(),
        "expected destinations to be populated for legacy configs"
    );
    assert_eq!(cfg.destinations.len(), 1);
    assert_eq!(cfg.destinations[0].id, "default");
    assert_eq!(cfg.destinations[0].path, cfg.backup_root);
    assert_eq!(cfg.destinations[0].label.as_deref(), Some("Primary"));
    assert_eq!(cfg.watched[0].destination_id, "default");

    validate(&cfg).expect("validate migrated config");
}
