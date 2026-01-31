use anyhow::{Context, Result};
use std::time::Duration;

/// Purpose: Retries an operation with a fixed backoff schedule.
///
/// Inputs: a label for error context, the retry delays, and the operation closure.
/// Outputs: the successful result or a detailed error after retries.
/// Ties to: hashing and copy operations for transient IO errors.
/// Side effects: Sleeps between retry attempts.
/// Why: improve resilience against temporary filesystem or hashing failures.
pub(super) fn retry_with_backoff<T, F>(label: &str, delays: &[Duration], mut op: F) -> Result<T>
where
    F: FnMut() -> Result<T>,
{
    if delays.is_empty() {
        return op()
            .with_context(|| format!("retry_with_backoff({}) single attempt failed", label));
    }
    let mut last_err = None;
    for (i, delay) in delays.iter().enumerate() {
        match op() {
            Ok(val) => return Ok(val),
            Err(e) => {
                last_err = Some(e);
                if i == delays.len() - 1 {
                    break;
                }
                std::thread::sleep(*delay);
            }
        }
    }
    Err(anyhow::anyhow!(
        "retry_with_backoff({}) failed after {} attempts: {}",
        label,
        delays.len(),
        last_err
            .map(|e| e.to_string())
            .unwrap_or_else(|| "unknown error".to_string())
    ))
}
