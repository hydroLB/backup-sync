use crate::commands::error::ErrorEnvelope;
use crate::commands::io_policy::run_blocking_io;
use anyhow::Context;
use backup_core::io::{run_with_policy, BlockingIoPolicy, CancellationFlag};
use backup_core::{backup::versioned, fs::snapshots::prepare_source_view, load_validated_config};
use fs2::free_space;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
/// Summary: Request payload controlling hardening checks.
///
/// Inputs: Deserialized from IPC.
///
/// Outputs: A typed request used by `hardening_check_cmd`.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: First-run wizard gating in the frontend.
///
/// Why this exists: Keep optional checks explicit so potentially privileged operations are opt-in.
pub struct HardeningCheckRequest {
    pub check_snapshots: bool,
    #[serde(default)]
    pub require_snapshots: bool,
}

#[derive(Debug, Clone, Serialize)]
/// Summary: Structured result describing a watched-path permission probe outcome.
///
/// Inputs: Derived from filesystem access tests.
///
/// Outputs: A serializable issue record.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: First-run wizard display and gating.
///
/// Why this exists: Provide actionable feedback on which path is blocking scheduling.
pub struct WatchedIssue {
    pub path: String,
    pub kind: String,
    pub issue: String,
}

#[derive(Debug, Clone, Serialize)]
/// Summary: Structured result describing a destination preflight check.
///
/// Inputs: Derived from store write probes and free-space checks.
///
/// Outputs: A serializable destination result.
///
/// Side effects: May create and remove a tiny probe file under the destination store.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: First-run wizard display and gating.
///
/// Why this exists: Backups should not begin scheduling when the destination is unwritable or nearly full.
pub struct DestinationHardening {
    pub id: String,
    pub path: String,
    pub ok: bool,
    pub free_bytes: Option<u64>,
    pub required_free_bytes: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
/// Summary: Structured result describing snapshot capability probing.
///
/// Inputs: Derived from best-effort snapshot preparation.
///
/// Outputs: A serializable snapshot probe result.
///
/// Side effects: May invoke OS snapshot tooling when enabled.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: First-run wizard optional snapshot validation.
///
/// Why this exists: Snapshot-backed scans reduce the chance of inconsistent versions on changing sources.
pub struct SnapshotHardening {
    pub checked: bool,
    pub supported: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
/// Summary: Report payload describing whether it is safe to enable scheduling.
///
/// Inputs: Derived from config and filesystem probes.
///
/// Outputs: A serializable hardening report.
///
/// Side effects: May create destination store directories and probe files; may run snapshot tooling when opted in.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: First-run wizard gating in Settings and Minimal UI.
///
/// Why this exists: Prevent enabling background writes before basic prerequisites are satisfied.
pub struct HardeningReport {
    pub ok: bool,
    pub message: String,
    pub watched_ok: Vec<String>,
    pub watched_issues: Vec<WatchedIssue>,
    pub destinations: Vec<DestinationHardening>,
    pub snapshots: SnapshotHardening,
}

#[tauri::command]
/// Summary: Run first-run hardening checks before enabling scheduling.
///
/// Inputs: A `HardeningCheckRequest` specifying whether to probe snapshot capability.
///
/// Outputs: A `HardeningReport` or an error envelope on config load/validation failures.
///
/// Side effects: Reads filesystem metadata; may create and remove a tiny destination probe file; may invoke OS snapshot tooling.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: First-run wizard and "enable running" gating.
///
/// Why this exists: Scheduling should only be enabled once the app can reliably read sources and write to the destination.
pub fn hardening_check_cmd(
    request: Option<HardeningCheckRequest>,
) -> Result<HardeningReport, ErrorEnvelope> {
    let req = request.unwrap_or(HardeningCheckRequest {
        check_snapshots: false,
        require_snapshots: false,
    });
    let cfg = load_validated_config().map_err(|e| {
        ErrorEnvelope::new(
            "CONFIG_LOAD",
            format!("hardening::hardening_check_cmd failed to load config: {e}"),
        )
    })?;

    let mut watched_ok: Vec<String> = Vec::new();
    let mut watched_issues: Vec<WatchedIssue> = Vec::new();
    let mut required_destination_ids: HashSet<&str> = HashSet::new();

    for w in cfg.watched.iter().filter(|w| w.enabled) {
        required_destination_ids.insert(w.destination_id.as_str());
        match probe_watched_path(&w.path, &w.kind) {
            Ok(()) => watched_ok.push(w.path.display().to_string()),
            Err(issue) => watched_issues.push(WatchedIssue {
                path: w.path.display().to_string(),
                kind: format!("{:?}", w.kind),
                issue,
            }),
        }
    }

    let destinations_by_id: HashMap<&str, &backup_core::config::model::Destination> = cfg
        .destinations
        .iter()
        .map(|d| (d.id.as_str(), d))
        .collect();
    let required_free_bytes = cfg
        .min_free_space_bytes
        .unwrap_or(0)
        .max(cfg.execution.free_space_safety_buffer_bytes);

    let mut destinations: Vec<DestinationHardening> = Vec::new();
    for id in required_destination_ids.iter() {
        let Some(dest) = destinations_by_id.get(id) else {
            destinations.push(DestinationHardening {
                id: id.to_string(),
                path: String::new(),
                ok: false,
                free_bytes: None,
                required_free_bytes,
                message: "missing destination config for watched path".to_string(),
            });
            continue;
        };
        destinations.push(probe_destination_store(id, &dest.path, required_free_bytes));
    }
    destinations.sort_by(|a, b| a.id.cmp(&b.id));

    let snapshots = if req.check_snapshots {
        probe_snapshots(&cfg, req.require_snapshots)
    } else {
        SnapshotHardening {
            checked: false,
            supported: false,
            message: "not checked".to_string(),
        }
    };

    let destinations_ok = destinations.iter().all(|d| d.ok);
    let watched_ok_all = watched_issues.is_empty();
    let snapshots_ok = if req.require_snapshots {
        snapshots.checked && snapshots.supported
    } else {
        true
    };
    let ok = destinations_ok && watched_ok_all && snapshots_ok;

    let message = if ok {
        "ok".to_string()
    } else {
        let mut parts: Vec<&str> = Vec::new();
        if !watched_ok_all {
            parts.push("watched paths not readable");
        }
        if !destinations_ok {
            parts.push("destination not writable or low space");
        }
        if !snapshots_ok {
            parts.push("snapshots required but unavailable");
        }
        parts.join("; ")
    };

    Ok(HardeningReport {
        ok,
        message,
        watched_ok,
        watched_issues,
        destinations,
        snapshots,
    })
}

/// Summary: Probe a watched path for basic readability.
///
/// Inputs: Filesystem path and watched kind.
///
/// Outputs: `Ok(())` when the path is accessible; otherwise an issue string.
///
/// Side effects: Reads filesystem metadata and may open the path.
///
/// Error handling: Returns a precise issue string for UI display.
///
/// Ties to other methods: Used by `hardening_check_cmd`.
///
/// Why this exists: Scheduling should not be enabled if the daemon cannot read the source.
fn probe_watched_path(
    path: &Path,
    kind: &backup_core::config::model::WatchedKind,
) -> Result<(), String> {
    if !path.exists() {
        return Err("missing".to_string());
    }
    if let Err(e) = run_blocking_io("gui::hardening::probe_watched_path metadata", || {
        fs::metadata(path).map(|_| ()).with_context(|| {
            format!(
                "hardening::probe_watched_path metadata failed for {:?}",
                path
            )
        })
    }) {
        return Err(format!("metadata failed: {e}"));
    }
    match kind {
        backup_core::config::model::WatchedKind::File => {
            run_blocking_io("gui::hardening::probe_watched_path file open", || {
                fs::File::open(path).map(|_| ()).with_context(|| {
                    format!("hardening::probe_watched_path open failed for {:?}", path)
                })
            })
            .map_err(|e| format!("open failed: {e}"))?;
        }
        backup_core::config::model::WatchedKind::Directory => {
            let mut rd = run_blocking_io("gui::hardening::probe_watched_path read_dir", || {
                fs::read_dir(path).with_context(|| {
                    format!(
                        "hardening::probe_watched_path read_dir failed for {:?}",
                        path
                    )
                })
            })
            .map_err(|e| format!("read_dir failed: {e}"))?;
            // Force at least one iteration attempt to surface permission issues consistently.
            if let Some(entry) = rd.next() {
                entry.map_err(|e| format!("read_dir iteration failed: {e}"))?;
            }
        }
    }
    Ok(())
}

/// Summary: Probe destination store writability and free space.
///
/// Inputs: destination id, destination root path, and required free space threshold.
///
/// Outputs: A `DestinationHardening` record.
///
/// Side effects: May create store directories and create/remove a tiny probe file.
///
/// Error handling: Never panics; records a `ok=false` message on failures.
///
/// Ties to other methods: Used by `hardening_check_cmd`.
///
/// Why this exists: Backups require creating `.backup_sync/v1` and writing blobs/manifests without disk-full failures.
fn probe_destination_store(
    id: &str,
    destination_root: &Path,
    required_free_bytes: u64,
) -> DestinationHardening {
    let path_str = destination_root.display().to_string();
    if destination_root.as_os_str().is_empty() {
        return DestinationHardening {
            id: id.to_string(),
            path: path_str,
            ok: false,
            free_bytes: None,
            required_free_bytes,
            message: "destination path is empty".to_string(),
        };
    }

    if destination_root.exists() && destination_root.is_file() {
        return DestinationHardening {
            id: id.to_string(),
            path: path_str,
            ok: false,
            free_bytes: None,
            required_free_bytes,
            message: "destination points to a file; choose a folder".to_string(),
        };
    }

    if let Err(e) = run_blocking_io(
        "gui::hardening::probe_destination_store create destination directory",
        || {
            fs::create_dir_all(destination_root).with_context(|| {
                format!(
                    "hardening::probe_destination_store failed creating destination directory {:?}",
                    destination_root
                )
            })
        },
    ) {
        return DestinationHardening {
            id: id.to_string(),
            path: path_str,
            ok: false,
            free_bytes: None,
            required_free_bytes,
            message: format!("cannot create destination directory: {e}"),
        };
    }

    let free_bytes = match free_space(destination_root) {
        Ok(free) => Some(free),
        Err(e) => {
            return DestinationHardening {
                id: id.to_string(),
                path: path_str,
                ok: false,
                free_bytes: None,
                required_free_bytes,
                message: format!("cannot read free space: {e}"),
            };
        }
    };

    if free_bytes.unwrap_or(0) < required_free_bytes {
        return DestinationHardening {
            id: id.to_string(),
            path: path_str,
            ok: false,
            free_bytes,
            required_free_bytes,
            message: format!("insufficient free space (need >= {required_free_bytes} bytes)"),
        };
    }

    let store_root = versioned::store_root_path(destination_root);
    if let Err(e) = run_blocking_io(
        "gui::hardening::probe_destination_store create store directory",
        || {
            fs::create_dir_all(&store_root).with_context(|| {
                format!(
                    "hardening::probe_destination_store failed creating store directory {:?}",
                    store_root
                )
            })
        },
    ) {
        return DestinationHardening {
            id: id.to_string(),
            path: path_str,
            ok: false,
            free_bytes,
            required_free_bytes,
            message: format!(
                "cannot create store directory {}: {e}",
                store_root.display()
            ),
        };
    }

    match write_probe_file(&store_root) {
        Ok(()) => DestinationHardening {
            id: id.to_string(),
            path: path_str,
            ok: true,
            free_bytes,
            required_free_bytes,
            message: "ok".to_string(),
        },
        Err(e) => DestinationHardening {
            id: id.to_string(),
            path: path_str,
            ok: false,
            free_bytes,
            required_free_bytes,
            message: e,
        },
    }
}

/// Summary: Write and fsync a small probe file under the store root.
///
/// Inputs: Store root directory path.
///
/// Outputs: `Ok(())` when write + fsync succeed.
///
/// Side effects: Creates and deletes a small file.
///
/// Error handling: Returns a contextual error string for UI display.
///
/// Ties to other methods: Used by `probe_destination_store`.
///
/// Why this exists: Free-space checks do not guarantee write permission; a real write probe catches mount/ACL issues.
fn write_probe_file(store_root: &Path) -> Result<(), String> {
    let policy = BlockingIoPolicy::single_attempt(BlockingIoPolicy::bootstrap_defaults().timeout);
    let token = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let probe_path: PathBuf = store_root.join(format!(".hardening_probe_{token}.bin"));
    let mut file = run_with_policy(
        "gui::hardening::write_probe_file create probe file",
        &policy,
        CancellationFlag::none(),
        || {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&probe_path)
                .with_context(|| {
                    format!(
                        "hardening::write_probe_file failed creating probe file {:?}",
                        probe_path
                    )
                })
        },
    )
    .map_err(|e| format!("cannot create probe file {}: {e}", probe_path.display()))?;
    run_with_policy(
        "gui::hardening::write_probe_file write+sync probe file",
        &policy,
        CancellationFlag::none(),
        || {
            file.write_all(b"backup_sync_probe").with_context(|| {
                format!("hardening::write_probe_file write failed {:?}", probe_path)
            })?;
            file.sync_all().with_context(|| {
                format!("hardening::write_probe_file fsync failed {:?}", probe_path)
            })?;
            Ok(())
        },
    )
    .map_err(|e| format!("cannot write probe file {}: {e}", probe_path.display()))?;
    drop(file);
    run_with_policy(
        "gui::hardening::write_probe_file remove probe file",
        &policy,
        CancellationFlag::none(),
        || {
            fs::remove_file(&probe_path).with_context(|| {
                format!(
                    "hardening::write_probe_file failed removing probe file {:?}",
                    probe_path
                )
            })
        },
    )
    .map_err(|e| format!("cannot remove probe file {}: {e}", probe_path.display()))?;
    Ok(())
}

/// Summary: Probe OS snapshot capability (best-effort) for the current config.
///
/// Inputs: Config and whether snapshots are required.
///
/// Outputs: A `SnapshotHardening` report describing support and a user-facing message.
///
/// Side effects: May invoke OS snapshot tooling.
///
/// Error handling: Never fails the overall hardening command; reports issues in the message.
///
/// Ties to other methods: Uses `prepare_source_view`, consistent with versioned backup scanning.
///
/// Why this exists: Snapshot-backed scans reduce inconsistencies when sources change during backup.
fn probe_snapshots(cfg: &backup_core::Config, require_snapshots: bool) -> SnapshotHardening {
    let Some(sample) = cfg.watched.iter().find(|w| w.enabled) else {
        return SnapshotHardening {
            checked: true,
            supported: false,
            message: "no watched paths configured".to_string(),
        };
    };
    let timeout_seconds = cfg
        .runtime
        .source_snapshot_timeout_seconds
        .min(cfg.runtime.hardening_snapshot_probe_timeout_seconds.max(1));
    match prepare_source_view(&sample.path, true, timeout_seconds) {
        Ok(view) => {
            if view.snapshot().is_some() {
                SnapshotHardening {
                    checked: true,
                    supported: true,
                    message: "supported".to_string(),
                }
            } else {
                let detail = view
                    .snapshot_error()
                    .unwrap_or("snapshot unavailable on this platform/path");
                SnapshotHardening {
                    checked: true,
                    supported: false,
                    message: if require_snapshots {
                        format!("unavailable (required): {detail}")
                    } else {
                        format!("unavailable: {detail}")
                    },
                }
            }
        }
        Err(e) => SnapshotHardening {
            checked: true,
            supported: false,
            message: if require_snapshots {
                format!("unavailable (required): {e:#}")
            } else {
                format!("unavailable: {e:#}")
            },
        },
    }
}
