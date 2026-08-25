# Configuration Reference

Last updated: 2026-08-23

Backup Sync reads TOML through a single validated loader shared by the daemon, CLI, and GUI.

## File locations

The default config directory comes from the platform's OS-native config location:

| Platform | Config directory |
|---|---|
| macOS | `~/Library/Application Support/backup_sync/` |
| Linux | `${XDG_CONFIG_HOME:-~/.config}/backup_sync/` |
| Windows | `%APPDATA%\backup_sync\` |

The directory contains:

- `config.toml`: operator-owned configuration
- `state.json`: runtime state, written atomically
- `key_v1.bin`: default encryption-key location when encryption is configured

Logs and the default backup root use the OS-native data directory (`~/Library/Application Support` on macOS, `${XDG_DATA_HOME:-~/.local/share}` on Linux, and `%APPDATA%` on Windows as resolved by the Rust `dirs` crate).

## Environment layering

| Variable | Meaning |
|---|---|
| `BACKUP_SYNC_CONFIG` | Select an explicit config file for reads and writes. The explicit file must exist before load. |
| `BACKUP_SYNC_INTERVAL_SECONDS` | Override the loaded schedule with a validated unsigned integer. |
| `BACKUP_SYNC_SAFE_MODE` | Override safe mode; accepts `true/false`, `1/0`, `yes/no`, or `on/off`. |

An explicit `BACKUP_SYNC_CONFIG` path is authoritative: GUI saves go back to the same file. Environment overrides are applied after TOML parsing and before validation.

## Minimal example

```toml
backup_root = "/Volumes/Backup/BackupSync"
interval_seconds = 1800
max_backups_per_file = 5
watched = []

[[destinations]]
id = "primary"
path = "/Volumes/Backup/BackupSync"
label = "Primary"
```

`backup_root` remains in the compatibility schema; versioned operations route each watched source through its `destination_id`. Destination IDs must be unique and every watched mapping must resolve to one.

## Complete example

This example shows every currently serialized setting and its default value where one exists. Replace paths before use.

```toml
backup_root = "/Volumes/Backup/BackupSync"
interval_seconds = 1800
max_backups_per_file = 5
skip_hidden = true
ignore_patterns = ["**/.DS_Store", "**/node_modules/**", "**/target/**", "**/tmp/**", "**/~$*"]
max_parallel_copies = 1
# max_bytes_per_second = 10485760
# min_free_space_bytes = 1073741824
safe_mode = false

[encryption]
enabled = false
# key_path = "/secure/offline/key_v1.bin"
# key_id = "generated-key-id"
blob_chunk_bytes = 65536

[compression]
enabled = false
blob_chunk_bytes = 65536
zstd_level = 3

[hashing]
buffer_bytes = 65536
timeout_seconds = 30

[execution]
copy_buffer_bytes = 65536
copy_timeout_seconds = 300
free_space_safety_buffer_bytes = 10485760
recent_activity_cap = 50
retry_delays_ms = [100, 200, 400, 800]
retry_jitter_pct = 0.2
blocking_io_backoff_poll_interval_ms = 50

[planning]
hash_check_interval = 5
max_plan_items = 20000
scan_timeout_seconds = 300
scan_capacity_multiplier = 16

[runtime]
prune_interval_cycles = 10
force_full_scan_interval_cycles = 24
verify_interval_seconds = 86400
scrub_full_interval_seconds = 604800
scrub_sample_blobs = 200
scrub_sample_versions_per_source = 2
watcher_debounce_seconds = 2
ipc_timeout_seconds = 5
ipc_request_max_bytes = 65536
ipc_response_max_bytes = 4194304
ipc_read_chunk_bytes = 8192
daemon_shutdown_join_timeout_seconds = 30
service_command_timeout_seconds = 15
service_command_retry_delay_ms = 300
service_command_poll_interval_ms = 50
source_snapshots_enabled = false
source_snapshot_timeout_seconds = 20
tray_tooltip_refresh_seconds = 10
tray_full_check_min_interval_seconds = 20
tray_full_check_timeout_seconds = 8
tray_low_space_warning_bytes = 2147483648
log_tail_lines = 200
log_tail_read_chunk_bytes = 8192
log_tail_max_bytes = 524288
simulation_sample_limit = 10
hardening_snapshot_probe_timeout_seconds = 5
gui_close_final_backup_debounce_ms = 300
gui_close_final_backup_reset_delay_seconds = 2
gui_quit_final_backup_timeout_seconds = 3
gui_daemon_stop_timeout_seconds = 3
gui_quit_force_exit_timeout_seconds = 8
replication_enabled = true
replication_mirror_manifests = true
replication_max_manifest_deletes_per_cycle = 500
large_deletion_keep_extra_threshold_ratio = 0.5
gui_start_hidden = false

[[destinations]]
id = "primary"
path = "/Volumes/Backup/BackupSync"
label = "Primary"
replicate_to = ["mirror"]

[[destinations]]
id = "mirror"
path = "/Volumes/BackupMirror/BackupSync"
label = "Mirror"
replicate_to = []

[[watched]]
path = "/Users/me/Documents"
kind = "Directory"
enabled = true
destination_id = "primary"
max_backups_per_file = 10
```

The legacy-named GUI lifecycle timeout fields remain serialized for compatibility but closing and quitting the current GUI do not run an implicit backup, force safe mode, stop the installed daemon, or force-exit the process.

## Destinations and replication

- Each watched source chooses one primary `destination_id`.
- A destination's `replicate_to` values name secondary destination IDs.
- Versioned backup, retention, restore, scrub, and replication operations use bounded cross-process leases per store.
- Replication validates the source store, repairs missing destination blobs/manifests, and publishes the replica index last.
- `replication_mirror_manifests` allows bounded removal of replica manifests no longer visible at the source; `replication_max_manifest_deletes_per_cycle` limits that deletion set.

Do not create replication cycles or overlapping physical destinations.

## Encryption and compression

Encryption and compression are independently optional. Blob names remain the SHA-256 of decoded plaintext, preserving deduplication and verification across encoding settings. Manifests remain plaintext. Generate a key with:

```bash
cargo run -p cli -- keygen
```

Back up and test the key separately before enabling encryption. Key loss is unrecoverable.

## Scheduling and desktop saves

The desktop app exposes a fixed 30-minute schedule and writes `interval_seconds = 1800` when saving configuration. Treat custom intervals as file/CLI-driven configuration. Watcher events can trigger work sooner; `force_full_scan_interval_cycles` bounds reliance on watcher delivery.

## Snapshots

Source snapshots are opt-in because platform providers may require elevated privileges. If snapshot creation fails, the engine records the failure and may fall back to a live source read; its content/hash invariant still prevents a blob from being published under the wrong digest.

## Validation and recovery

Invalid configuration is reported in the desktop shell with Retry instead of silently exiting. Fix the file, then retry. Use these read-oriented checks before enabling writes:

```bash
cargo run -p cli -- doctor
cargo run -p cli -- run-once --dry-run
```

For storage semantics, see [Storage](storage.md). For operational tuning, see [Performance tuning](performance-tuning.md).
