use crate::commands::destination;
use crate::commands::status::get_status;
use crate::commands::{access, hardening};
use crate::tray;
use chrono::Utc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::async_runtime;
use tracing::warn;

static FULL_CHECK_IN_FLIGHT: AtomicBool = AtomicBool::new(false);
static HEALTH_CACHE: OnceLock<Mutex<TrayHealthCache>> = OnceLock::new();
static HEALTH_CACHE_INIT: OnceLock<()> = OnceLock::new();

#[derive(Clone, Debug)]
struct TrayHealthSnapshot {
    startup_hardening_error: Option<String>,
    access_probe: Option<access::AccessProbe>,
    destination_issues: Vec<String>,
}

#[derive(Clone, Debug)]
struct TrayHealthCache {
    last_full_check: Option<Instant>,
    last_snapshot: TrayHealthSnapshot,
}

impl Default for TrayHealthCache {
    /// Summary: default orchestrates this method's core behavior.
    ///
    /// Inputs: Method parameters and required receiver state.
    ///
    /// Outputs: Return value and observable result for callers.
    ///
    /// Side effects: None beyond this method's explicit operations.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: Invoked by and composes with adjacent module methods.
    ///
    /// Why this exists: Keeps this behavior isolated, testable, and reusable.
    fn default() -> Self {
        Self {
            last_full_check: None,
            last_snapshot: TrayHealthSnapshot {
                startup_hardening_error: None,
                access_probe: None,
                destination_issues: Vec::new(),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TraySeverity {
    Normal,
    Warning,
    Error,
}

impl TraySeverity {
    /// Summary: label orchestrates this method's core behavior.
    ///
    /// Inputs: Method parameters and required receiver state.
    ///
    /// Outputs: Return value and observable result for callers.
    ///
    /// Side effects: None beyond this method's explicit operations.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: Invoked by and composes with adjacent module methods.
    ///
    /// Why this exists: Keeps this behavior isolated, testable, and reusable.
    fn label(self) -> &'static str {
        match self {
            TraySeverity::Normal => "Normal",
            TraySeverity::Warning => "Warning",
            TraySeverity::Error => "Error",
        }
    }
}

impl TraySeverity {
    /// Summary: max orchestrates this method's core behavior.
    ///
    /// Inputs: Method parameters and required receiver state.
    ///
    /// Outputs: Return value and observable result for callers.
    ///
    /// Side effects: None beyond this method's explicit operations.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: Invoked by and composes with adjacent module methods.
    ///
    /// Why this exists: Keeps this behavior isolated, testable, and reusable.
    fn max(self, other: TraySeverity) -> TraySeverity {
        use TraySeverity::*;
        match (self, other) {
            (Error, _) | (_, Error) => Error,
            (Warning, _) | (_, Warning) => Warning,
            _ => Normal,
        }
    }
}

/// Summary: format_last_sync_label orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
fn format_last_sync_label(last_run_ts: Option<i64>) -> String {
    match last_run_ts {
        None => "Last sync: Never".to_string(),
        Some(ts) => {
            let now = Utc::now().timestamp();
            let mins = (now - ts).max(0) / 60;
            if mins < 120 {
                format!("Last sync: {}m ago", mins)
            } else {
                format!("Last sync: {}h ago", mins / 60)
            }
        }
    }
}

/// Summary: Return cached health snapshot, refreshing it in the background when stale.
///
/// Inputs: none.
///
/// Outputs: Most recent snapshot.
///
/// Side effects: May spawn a blocking refresh task.
///
/// Error handling: Refresh failures are stored as error strings; stale values may be used.
///
/// Ties to other methods: Used by `update_tray_tooltip` for tray severity calculation.
///
/// Why this exists: Keep tray updates responsive even when filesystem checks are slow.
async fn read_or_refresh_health_snapshot() -> TrayHealthSnapshot {
    let runtime = super::tuning::resolve_runtime_tuning();
    let full_check_min_interval =
        Duration::from_secs(runtime.tray_full_check_min_interval_seconds.max(1));
    let full_check_timeout = Duration::from_secs(runtime.tray_full_check_timeout_seconds.max(1));
    let cache = HEALTH_CACHE.get_or_init(|| Mutex::new(TrayHealthCache::default()));
    HEALTH_CACHE_INIT.get_or_init(|| ());

    let (should_refresh, is_first, snapshot) = {
        let guard = match cache.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        let is_first = guard.last_full_check.is_none();
        let stale = guard
            .last_full_check
            .map(|t| t.elapsed() >= full_check_min_interval)
            .unwrap_or(true);
        (stale, is_first, guard.last_snapshot.clone())
    };

    if is_first {
        // Full check on startup: do one refresh eagerly so the tray reflects reality immediately,
        // but keep it off the async executor thread and bounded by a timeout.
        let refreshed = tokio::time::timeout(full_check_timeout, async {
            match tokio::task::spawn_blocking(run_full_health_check).await {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    warn!(
                        error = %error,
                        "app::tray_tooltip::read_or_refresh_health_snapshot startup health task failed"
                    );
                    TrayHealthSnapshot {
                        startup_hardening_error: Some("Safety checks: task failed".to_string()),
                        access_probe: None,
                        destination_issues: Vec::new(),
                    }
                }
            }
        })
        .await
        .unwrap_or_else(|error| {
            warn!(
                error = %error,
                "app::tray_tooltip::read_or_refresh_health_snapshot startup health task timed out"
            );
            TrayHealthSnapshot {
                startup_hardening_error: Some("Safety checks: timed out".to_string()),
                access_probe: None,
                destination_issues: Vec::new(),
            }
        });

        let mut guard = match cache.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.last_full_check = Some(Instant::now());
        guard.last_snapshot = refreshed.clone();
        return refreshed;
    }

    if should_refresh && !FULL_CHECK_IN_FLIGHT.swap(true, Ordering::Relaxed) {
        async_runtime::spawn(async move {
            let refreshed = tokio::time::timeout(full_check_timeout, async {
                match tokio::task::spawn_blocking(run_full_health_check).await {
                    Ok(snapshot) => snapshot,
                    Err(error) => {
                        warn!(
                            error = %error,
                            "app::tray_tooltip::read_or_refresh_health_snapshot refresh health task failed"
                        );
                        TrayHealthSnapshot {
                            startup_hardening_error: Some("Safety checks: task failed".to_string()),
                            access_probe: None,
                            destination_issues: Vec::new(),
                        }
                    }
                }
            })
            .await
            .unwrap_or_else(|error| {
                warn!(
                    error = %error,
                    "app::tray_tooltip::read_or_refresh_health_snapshot refresh health task timed out"
                );
                TrayHealthSnapshot {
                    startup_hardening_error: Some("Safety checks: timed out".to_string()),
                    access_probe: None,
                    destination_issues: Vec::new(),
                }
            });

            let cache = HEALTH_CACHE.get_or_init(|| Mutex::new(TrayHealthCache::default()));
            let mut guard = match cache.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            guard.last_full_check = Some(Instant::now());
            guard.last_snapshot = refreshed;
            FULL_CHECK_IN_FLIGHT.store(false, Ordering::Relaxed);
        });
    }

    snapshot
}

/// Summary: Perform a full health check (hardening, access, destination writability).
///
/// Inputs: none.
///
/// Outputs: A snapshot of checks used by tray severity.
///
/// Side effects: Reads config and probes filesystem paths.
///
/// Error handling: Converts failures into user-facing strings.
///
/// Ties to other methods: Called by `read_or_refresh_health_snapshot` in a blocking task.
///
/// Why this exists: Keep tray status aligned with current disk and path health.
fn run_full_health_check() -> TrayHealthSnapshot {
    let startup_hardening_error =
        match hardening::hardening_check_cmd(Some(hardening::HardeningCheckRequest {
            check_snapshots: true,
            require_snapshots: false,
        })) {
            Ok(report) => {
                if report.ok {
                    None
                } else {
                    Some(format!("Safety checks: {}", report.message))
                }
            }
            Err(err) => Some(format!("Safety checks: {}", err.message)),
        };

    let access_probe = match access::test_access_cmd() {
        Ok(probe) => Some(probe),
        Err(error) => {
            warn!(
                error = %error.message,
                "app::tray_tooltip::run_full_health_check access probe failed"
            );
            None
        }
    };

    let destination_issues = match backup_core::load_validated_config() {
        Ok(cfg) => cfg
            .destinations
            .iter()
            .filter_map(|d| {
                let check = match destination::check_destination_cmd(d.path.display().to_string()) {
                    Ok(check) => check,
                    Err(error) => {
                        warn!(
                            destination = %d.path.display(),
                            error = %error.message,
                            "app::tray_tooltip::run_full_health_check destination check failed"
                        );
                        return Some(format!("{}: {}", d.path.display(), error.message));
                    }
                };
                if check.writable {
                    None
                } else {
                    let name = d
                        .label
                        .as_deref()
                        .map(|label| label.trim())
                        .filter(|label| !label.is_empty())
                        .map(|label| label.to_string())
                        .unwrap_or_else(|| d.path.display().to_string());
                    Some(format!("{name}: {}", check.message))
                }
            })
            .collect::<Vec<_>>(),
        Err(error) => {
            warn!(
                error = %error,
                "app::tray_tooltip::run_full_health_check failed to load config"
            );
            Vec::new()
        }
    };

    TrayHealthSnapshot {
        startup_hardening_error,
        access_probe,
        destination_issues,
    }
}

/// Summary: Refreshes the system tray tooltip using the latest daemon status.
///
/// Inputs: the Tauri app handle.
///
/// Outputs: `()` after attempting to set the tooltip.
///
/// Side effects: Performs IPC calls and updates the tray tooltip.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: tray refresh loops and IPC status calls.
///
/// Why this exists: keep tray feedback aligned with daemon health.
pub(crate) async fn update_tray_tooltip(handle: &tauri::AppHandle) {
    let runtime = super::tuning::resolve_runtime_tuning();
    let low_space_warning_bytes = runtime.tray_low_space_warning_bytes.max(1);
    let health = read_or_refresh_health_snapshot().await;

    match get_status().await {
        Ok(status) => {
            let mut severity = TraySeverity::Normal;
            let mut parts: Vec<String> = vec!["Online".into()];
            parts.push(format!("Status: {}", TraySeverity::Normal.label()));

            if health.startup_hardening_error.is_some() {
                severity = TraySeverity::Error;
            }
            if status.destination_paused {
                severity = TraySeverity::Error;
            }
            if status.safe_mode {
                severity = severity.max(TraySeverity::Warning);
            }
            if status.last_safety_warning.is_some() {
                severity = severity.max(TraySeverity::Warning);
            }
            if !health.destination_issues.is_empty() {
                severity = TraySeverity::Error;
            }
            if let Some(free) = status.free_bytes {
                if free < low_space_warning_bytes {
                    severity = severity.max(TraySeverity::Warning);
                }
            }
            if let Some(access) = health.access_probe.as_ref() {
                if !access.watched_unwritable.is_empty() {
                    severity = TraySeverity::Error;
                } else if !access.watched_missing.is_empty() {
                    severity = severity.max(TraySeverity::Warning);
                }
            }

            parts[1] = format!("Status: {}", severity.label());
            if let Some(free) = status.free_bytes {
                let gb = (free as f64) / (1024.0 * 1024.0 * 1024.0);
                parts.push(format!("{:.1} GB free", gb));
                if free < low_space_warning_bytes {
                    parts.push("LOW SPACE".into());
                }
            }
            if status.safe_mode {
                parts.push("SAFE MODE".into());
            }
            if let Some(w) = status.last_safety_warning.as_ref() {
                parts.push(w.message.clone());
            }
            if let Some(issue) = health.startup_hardening_error {
                parts.push(issue);
            }
            if !health.destination_issues.is_empty() {
                parts.push(format!(
                    "Destinations: {}",
                    health
                        .destination_issues
                        .iter()
                        .take(2)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(" • ")
                ));
            }
            let label = format!("Backup Sync • {}", parts.join(" • "));
            if let Err(error) = handle.tray_handle().set_tooltip(&label) {
                warn!(
                    error = %error,
                    "app::tray_tooltip::update_tray_tooltip failed setting tray tooltip"
                );
            }
            if let Err(error) = handle
                .tray_handle()
                .get_item(tray::STATUS_LINE)
                .set_title(format!("Status: {}", severity.label()))
            {
                warn!(
                    error = %error,
                    "app::tray_tooltip::update_tray_tooltip failed setting status tray item"
                );
            }
            if let Err(error) = handle
                .tray_handle()
                .get_item(tray::LAST_SYNC_LINE)
                .set_title(format_last_sync_label(status.last_run_ts))
            {
                warn!(
                    error = %error,
                    "app::tray_tooltip::update_tray_tooltip failed setting last-sync tray item"
                );
            }
        }
        Err(error) => {
            warn!(
                error = %error.message,
                "app::tray_tooltip::update_tray_tooltip failed getting daemon status"
            );
            if let Err(set_error) = handle.tray_handle().set_tooltip("Backup Sync • Offline") {
                warn!(
                    error = %set_error,
                    "app::tray_tooltip::update_tray_tooltip failed setting offline tray tooltip"
                );
            }
            if let Err(set_error) = handle
                .tray_handle()
                .get_item(tray::STATUS_LINE)
                .set_title("Status: Error (Offline)")
            {
                warn!(
                    error = %set_error,
                    "app::tray_tooltip::update_tray_tooltip failed setting offline status tray item"
                );
            }
            if let Err(set_error) = handle
                .tray_handle()
                .get_item(tray::LAST_SYNC_LINE)
                .set_title("Last sync: —")
            {
                warn!(
                    error = %set_error,
                    "app::tray_tooltip::update_tray_tooltip failed setting offline last-sync tray item"
                );
            }
        }
    }
}
