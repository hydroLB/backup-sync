use crate::config::model::{Config, ExecutionTuning};
use anyhow::{Context, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Summary: Runtime policy for blocking I/O operations.
///
/// Inputs: timeout, retry delays, and backoff poll interval values.
///
/// Outputs: A reusable policy object consumed by `run_with_policy`.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `run_with_policy`, config loading/saving, and state persistence paths.
///
/// Why this exists: Keep timeout and retry semantics explicit and consistent across blocking I/O call sites.
#[derive(Debug, Clone)]
pub struct BlockingIoPolicy {
    pub timeout: Duration,
    pub retry_delays: Vec<Duration>,
    pub backoff_poll_interval: Duration,
}

impl BlockingIoPolicy {
    /// Summary: Builds a single-attempt I/O policy with an explicit timeout.
    ///
    /// Inputs: Timeout duration.
    ///
    /// Outputs: A `BlockingIoPolicy` with retries disabled.
    ///
    /// Side effects: None.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: call sites where retries could produce duplicate or unsafe side effects.
    ///
    /// Why this exists: Some I/O paths must be bounded and cancellable but should not retry automatically.
    pub fn single_attempt(timeout: Duration) -> Self {
        Self {
            timeout: timeout.max(Duration::from_secs(1)),
            retry_delays: Vec::new(),
            backoff_poll_interval: Duration::from_millis(
                crate::config::model::defaults::default_blocking_io_backoff_poll_interval_ms(),
            ),
        }
    }

    /// Summary: Builds an I/O policy from full config execution tuning.
    ///
    /// Inputs: Loaded config.
    ///
    /// Outputs: A `BlockingIoPolicy` derived from central tuning knobs.
    ///
    /// Side effects: None.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: `from_execution_tuning` and blocking I/O wrappers.
    ///
    /// Why this exists: Avoid hardcoded timeout and retry values in runtime code paths.
    pub fn from_config(cfg: &Config) -> Self {
        Self::from_execution_tuning(&cfg.execution)
    }

    /// Summary: Builds an I/O policy from execution tuning only.
    ///
    /// Inputs: Execution tuning from config or defaults.
    ///
    /// Outputs: A `BlockingIoPolicy`.
    ///
    /// Side effects: None.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: `from_config` and `bootstrap_defaults`.
    ///
    /// Why this exists: Some callers only have execution tuning and still need shared I/O policy behavior.
    pub fn from_execution_tuning(tuning: &ExecutionTuning) -> Self {
        Self {
            timeout: Duration::from_secs(tuning.copy_timeout_seconds.max(1)),
            retry_delays: tuning.retry_delays(),
            backoff_poll_interval: Duration::from_millis(
                tuning.blocking_io_backoff_poll_interval_ms.max(1),
            ),
        }
    }

    /// Summary: Builds a bootstrap policy for call sites that run before config is loaded.
    ///
    /// Inputs: none.
    ///
    /// Outputs: A `BlockingIoPolicy` from default config knobs.
    ///
    /// Side effects: None.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: config/state bootstrap paths.
    ///
    /// Why this exists: Early startup I/O still needs explicit timeout and retry behavior.
    pub fn bootstrap_defaults() -> Self {
        let timeout_secs = crate::config::model::defaults::default_copy_timeout_seconds().max(1);
        let retry_delays = crate::config::model::defaults::default_retry_delays_ms()
            .into_iter()
            .map(Duration::from_millis)
            .collect();
        Self {
            timeout: Duration::from_secs(timeout_secs),
            retry_delays,
            backoff_poll_interval: Duration::from_millis(
                crate::config::model::defaults::default_blocking_io_backoff_poll_interval_ms(),
            ),
        }
    }
}

/// Summary: Optional cancellation flag used by blocking I/O policy checks.
///
/// Inputs: Optional shared `AtomicBool`.
///
/// Outputs: A wrapper that `run_with_policy` can query for cancellation.
///
/// Side effects: Reads atomic state during I/O loops.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `run_with_policy`.
///
/// Why this exists: Let long-running blocking operations stop early during shutdown or cancellation.
#[derive(Clone, Copy, Debug, Default)]
pub struct CancellationFlag<'a> {
    flag: Option<&'a AtomicBool>,
}

impl<'a> CancellationFlag<'a> {
    /// Summary: Creates a `CancellationFlag` with no cancellation source.
    ///
    /// Inputs: none.
    ///
    /// Outputs: A no-op cancellation wrapper.
    ///
    /// Side effects: None.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: `run_with_policy`.
    ///
    /// Why this exists: Keep cancellation behavior explicit even when a caller does not support cancellation.
    pub fn none() -> Self {
        Self { flag: None }
    }

    /// Summary: Creates a `CancellationFlag` backed by an atomic boolean.
    ///
    /// Inputs: Shared `AtomicBool` reference.
    ///
    /// Outputs: A cancellation wrapper linked to the provided flag.
    ///
    /// Side effects: Reads atomic state during I/O loops.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: `run_with_policy`.
    ///
    /// Why this exists: Provide low-overhead cooperative cancellation for blocking paths.
    pub fn from_atomic(flag: &'a AtomicBool) -> Self {
        Self { flag: Some(flag) }
    }

    /// Summary: Returns whether cancellation was requested.
    ///
    /// Inputs: The wrapped atomic flag.
    ///
    /// Outputs: `true` when cancellation is requested.
    ///
    /// Side effects: Reads atomic state.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: Internal helper used by `run_with_policy`.
    ///
    /// Why this exists: Keep cancellation checks consistent in one place.
    fn is_cancelled(&self) -> bool {
        self.flag
            .map(|f| f.load(Ordering::Relaxed))
            .unwrap_or(false)
    }
}

/// Summary: Executes a fallible blocking operation with timeout, retry, and cancellation checks.
///
/// Inputs: operation label, policy, cancellation flag, and operation closure.
///
/// Outputs: The successful operation result or a contextual error.
///
/// Side effects: Sleeps between retry attempts and evaluates cancellation checks.
///
/// Error handling: Returns explicit timeout/cancellation errors and preserves last operation failure context.
///
/// Ties to other methods: Used by config/state/key/service IO call sites and other blocking boundaries.
///
/// Why this exists: Ensure all blocking I/O paths use explicit and centralized resilience behavior.
pub fn run_with_policy<T, F>(
    label: &str,
    policy: &BlockingIoPolicy,
    cancellation: CancellationFlag<'_>,
    mut op: F,
) -> Result<T>
where
    F: FnMut() -> Result<T>,
{
    let start = Instant::now();
    let mut last_err: Option<anyhow::Error> = None;
    let max_attempts = policy.retry_delays.len() + 1;

    for attempt_idx in 0..max_attempts {
        ensure_active(label, policy, start, cancellation).with_context(|| {
            format!(
                "{} preflight failed before attempt {}",
                label,
                attempt_idx + 1
            )
        })?;

        match op() {
            Ok(value) => return Ok(value),
            Err(error) => {
                last_err = Some(error);
                if attempt_idx >= policy.retry_delays.len() {
                    break;
                }
                let delay = policy.retry_delays[attempt_idx];
                sleep_with_checks(label, policy, start, cancellation, delay).with_context(
                    || {
                        format!(
                            "{} backoff interrupted before attempt {}",
                            label,
                            attempt_idx + 2
                        )
                    },
                )?;
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("{label} failed with unknown error")))
        .with_context(|| format!("{label} failed after {max_attempts} attempts"))
}

/// Summary: Verifies that timeout and cancellation conditions still allow work.
///
/// Inputs: operation label, policy, start instant, and cancellation flag.
///
/// Outputs: `Ok(())` when operation may continue.
///
/// Side effects: Reads atomic cancellation state and wall clock.
///
/// Error handling: Returns explicit timeout or cancellation errors.
///
/// Ties to other methods: Internal helper used by `run_with_policy` and backoff sleeps.
///
/// Why this exists: Keep termination checks centralized and consistent.
fn ensure_active(
    label: &str,
    policy: &BlockingIoPolicy,
    start: Instant,
    cancellation: CancellationFlag<'_>,
) -> Result<()> {
    if cancellation.is_cancelled() {
        anyhow::bail!("{label} cancelled");
    }
    if start.elapsed() > policy.timeout {
        anyhow::bail!("{label} timed out after {:?}", policy.timeout);
    }
    Ok(())
}

/// Summary: Sleeps for retry backoff while enforcing timeout and cancellation checks.
///
/// Inputs: operation label, policy, start instant, cancellation flag, and backoff delay.
///
/// Outputs: `Ok(())` when the full delay elapsed without cancellation or timeout.
///
/// Side effects: Sleeps in short intervals to stay responsive to cancellation.
///
/// Error handling: Returns explicit timeout or cancellation errors.
///
/// Ties to other methods: Internal helper used by `run_with_policy`.
///
/// Why this exists: Retry loops should not block shutdown or exceed time bounds while sleeping.
fn sleep_with_checks(
    label: &str,
    policy: &BlockingIoPolicy,
    start: Instant,
    cancellation: CancellationFlag<'_>,
    delay: Duration,
) -> Result<()> {
    let backoff_poll = policy.backoff_poll_interval.max(Duration::from_millis(1));
    let sleep_start = Instant::now();
    while sleep_start.elapsed() < delay {
        ensure_active(label, policy, start, cancellation)?;
        let remaining = delay.saturating_sub(sleep_start.elapsed());
        std::thread::sleep(remaining.min(backoff_poll));
    }
    Ok(())
}
