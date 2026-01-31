use crate::commands::error::ErrorEnvelope;
use backup_core::config::model::RuntimeTuning;
use backup_core::load_config;
use std::env;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::State;
use tracing::warn;

const AUTH_PASSPHRASE_ENV: &str = "BACKUP_SYNC_PASSPHRASE";

/// Purpose: Loads runtime tuning for GUI auth behavior.
///
/// Inputs: none.
/// Outputs: runtime tuning from config or defaults.
/// Ties to: auth unlock timing and config driven security behavior.
/// Side effects: Reads config from disk and logs warnings on fallback.
/// Why: keep unlock timing configurable without hardcoded constants.
fn resolve_runtime_tuning() -> RuntimeTuning {
    match load_config() {
        Ok(cfg) => cfg.runtime,
        Err(e) => {
            warn!(
                "auth::resolve_runtime_tuning failed to load config; using defaults: {}",
                e
            );
            RuntimeTuning::default()
        }
    }
}

/// Purpose: Loads the required passphrase from the environment.
///
/// Inputs: none.
/// Outputs: the passphrase string.
/// Ties to: auth unlock validation.
/// Side effects: Reads process environment variables.
/// Why: centralize passphrase retrieval with consistent errors.
fn load_passphrase() -> Result<String, ErrorEnvelope> {
    let passphrase = env::var(AUTH_PASSPHRASE_ENV).map_err(|e| {
        ErrorEnvelope::new(
            "AUTH_MISSING",
            format!(
                "auth::load_passphrase {} not set: {}",
                AUTH_PASSPHRASE_ENV, e
            ),
        )
    })?;
    if passphrase.trim().is_empty() {
        return Err(ErrorEnvelope::new(
            "AUTH_MISSING",
            format!("auth::load_passphrase {} is empty", AUTH_PASSPHRASE_ENV),
        ));
    }
    Ok(passphrase)
}

/// Purpose: Ensures the passphrase is configured for auth.
///
/// Inputs: none.
/// Outputs: `Ok(())` when the passphrase is configured.
/// Ties to: auth status checks and access guards.
/// Side effects: Reads process environment variables.
/// Why: prevent silent auth bypass when no passphrase is set.
pub(crate) fn ensure_passphrase_configured() -> Result<(), ErrorEnvelope> {
    load_passphrase().map(|_| ())
}

/// Purpose: Compare two strings in constant time.
///
/// Inputs: candidate and expected strings.
/// Outputs: `true` when the strings match exactly.
/// Ties to: passphrase validation.
/// Side effects: None.
/// Why: reduce timing side channels during auth checks.
fn constant_time_eq(candidate: &str, expected: &str) -> bool {
    if candidate.len() != expected.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (a, b) in candidate.as_bytes().iter().zip(expected.as_bytes().iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

#[derive(Default)]
/// Purpose: Shared session auth state for GUI commands.
///
/// Inputs: lock state stored in an `Arc<Mutex<_>>`.
/// Outputs: a shared auth handle.
/// Ties to: GUI auth enforcement and command guards.
/// Side effects: None.
/// Why: centralize session unlock tracking.
pub struct SessionAuth {
    pub inner: Arc<Mutex<AuthState>>,
}

#[derive(Default)]
/// Purpose: Auth state stored inside the session mutex.
///
/// Inputs: the unlock timestamp.
/// Outputs: a simple auth state container.
/// Ties to: session unlock timing.
/// Side effects: None.
/// Why: separate the data from the session handle.
pub struct AuthState {
    pub unlocked_until: Option<Instant>,
}

#[allow(dead_code)]
impl SessionAuth {
    /// Purpose: Returns true when the session is currently unlocked.
    ///
    /// Inputs: the current session state.
    /// Outputs: a boolean indicating whether unlock is active.
    /// Ties to: auth gating logic for GUI commands.
    /// Side effects: None.
    /// Why: provide a simple unlock check for command guards.
    pub(crate) fn is_unlocked(&self) -> bool {
        let guard = self.inner.lock().unwrap_or_else(|e| {
            warn!("auth::SessionAuth::is_unlocked mutex poisoned: {}", e);
            e.into_inner()
        });
        match guard.unlocked_until {
            Some(ts) => ts > Instant::now(),
            None => false,
        }
    }

    /// Purpose: Returns the remaining unlock time in seconds if active.
    ///
    /// Inputs: the current session state.
    /// Outputs: the remaining seconds or None if locked.
    /// Ties to: auth status reporting for the UI.
    /// Side effects: None.
    /// Why: surface time left when session unlock is temporary.
    pub(crate) fn seconds_left(&self) -> Option<u64> {
        let guard = self.inner.lock().unwrap_or_else(|e| {
            warn!("auth::SessionAuth::seconds_left mutex poisoned: {}", e);
            e.into_inner()
        });
        guard
            .unlocked_until
            .and_then(|ts| ts.checked_duration_since(Instant::now()))
            .map(|d| d.as_secs())
    }

    /// Purpose: Unlocks the session for a fixed duration.
    ///
    /// Inputs: none.
    /// Outputs: `()` after updating the unlock deadline.
    /// Ties to: auth commands when the user unlocks.
    /// Side effects: None.
    /// Why: allow time limited access to privileged actions.
    pub(crate) fn unlock(&self) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| {
            warn!("auth::SessionAuth::unlock mutex poisoned: {}", e);
            e.into_inner()
        });
        let runtime = resolve_runtime_tuning();
        guard.unlocked_until =
            Some(Instant::now() + Duration::from_secs(runtime.auth_unlock_seconds));
    }

    /// Purpose: Locks the session immediately.
    ///
    /// Inputs: none.
    /// Outputs: `()` after clearing the unlock deadline.
    /// Ties to: auth commands when the user locks the session.
    /// Side effects: None.
    /// Why: revoke privileged access immediately when requested.
    pub(crate) fn lock(&self) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| {
            warn!("auth::SessionAuth::lock mutex poisoned: {}", e);
            e.into_inner()
        });
        guard.unlocked_until = None;
    }
}

#[tauri::command]
/// Purpose: Returns the current auth status for the UI.
///
/// Inputs: the session state.
/// Outputs: a tuple of (unlocked, seconds_left).
/// Ties to: GUI auth status checks.
/// Side effects: Reads shared auth state.
/// Why: allow the UI to show lock status.
pub fn auth_status_cmd(_state: State<SessionAuth>) -> Result<(bool, Option<u64>), ErrorEnvelope> {
    ensure_passphrase_configured()?;
    Ok((_state.is_unlocked(), _state.seconds_left()))
}

#[tauri::command]
/// Purpose: Unlocks the session with a passcode.
///
/// Inputs: the passcode, optional correlation id, and auth state.
/// Outputs: a status message or an error envelope.
/// Ties to: GUI unlock flows.
/// Side effects: Mutates shared auth state to unlock the session.
/// Why: allow UI to unlock privileged actions.
pub fn unlock_session_cmd(
    _passcode: String,
    correlation_id: Option<String>,
    state: State<SessionAuth>,
) -> Result<String, ErrorEnvelope> {
    let expected = load_passphrase()?;
    if !constant_time_eq(&_passcode, &expected) {
        let cid = crate::commands::correlation::cid("auth", correlation_id);
        return Err(ErrorEnvelope::new(
            "AUTH_INVALID",
            format!(
                "auth::unlock_session_cmd invalid passphrase for cid {}",
                cid
            ),
        ));
    }
    state.unlock();
    let runtime = resolve_runtime_tuning();
    Ok(format!(
        "Session unlocked for {} seconds.",
        runtime.auth_unlock_seconds
    ))
}

#[tauri::command]
/// Purpose: Locks the session immediately.
///
/// Inputs: the auth state handle.
/// Outputs: `Ok(())` when the session is locked.
/// Ties to: GUI lock actions.
/// Side effects: Mutates shared auth state to lock the session.
/// Why: allow the UI to revoke privileged access.
pub fn lock_session_cmd(state: State<SessionAuth>) -> Result<(), ErrorEnvelope> {
    ensure_passphrase_configured()?;
    state.lock();
    Ok(())
}
