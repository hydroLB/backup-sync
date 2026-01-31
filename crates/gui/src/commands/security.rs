use crate::commands::auth::ensure_passphrase_configured;
use crate::commands::auth::SessionAuth;
use crate::commands::correlation;
use crate::commands::error::ErrorEnvelope;
use tauri::State;

/// Purpose: Ensures the session is unlocked, using a Tauri state handle.
///
/// Inputs: the session state and optional correlation id.
/// Outputs: `Ok(())` when access is allowed.
/// Ties to: guarded GUI commands.
/// Side effects: None.
/// Why: centralize auth checks for GUI commands.
pub fn ensure_unlocked(
    state: &State<SessionAuth>,
    correlation_id: Option<String>,
) -> Result<(), ErrorEnvelope> {
    ensure_session_unlocked(&*state, correlation_id)
}

/// Purpose: Ensures the provided session auth is unlocked.
///
/// Inputs: the session auth reference and optional correlation id.
/// Outputs: `Ok(())` when access is allowed.
/// Ties to: auth checks where a SessionAuth reference is already available.
/// Side effects: None.
/// Why: allow non State based auth checks with explicit gating.
pub fn ensure_session_unlocked(
    auth: &SessionAuth,
    correlation_id: Option<String>,
) -> Result<(), ErrorEnvelope> {
    ensure_passphrase_configured()?;
    if auth.is_unlocked() {
        return Ok(());
    }
    let cid = correlation::cid("auth", correlation_id);
    Err(ErrorEnvelope::new(
        "AUTH_LOCKED",
        format!(
            "security::ensure_session_unlocked locked for cid {}; unlock required",
            cid
        ),
    ))
}

/// Purpose: Builds a correlation id for GUI command tracing.
///
/// Inputs: a prefix and optional incoming id.
/// Outputs: a correlation id string.
/// Ties to: logging and diagnostics.
/// Side effects: Reads system time.
/// Why: standardize correlation ids across commands.
pub fn cid(prefix: &str, incoming: Option<String>) -> String {
    correlation::cid(prefix, incoming)
}
