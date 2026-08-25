use crate::config::model::{Config, ExecutionTuning};
use anyhow::{Context, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Keep timeout and retry semantics explicit and consistent across blocking I/O call sites.
#[derive(Debug, Clone)]
pub struct BlockingIoPolicy {
    pub timeout: Duration,
    pub retry_delays: Vec<Duration>,
    pub backoff_poll_interval: Duration,
}

impl BlockingIoPolicy {
    /// Some I/O paths must be bounded and cancellable but should not retry automatically.
    pub fn single_attempt(timeout: Duration) -> Self {
        Self {
            timeout: timeout.max(Duration::from_secs(1)),
            retry_delays: Vec::new(),
            backoff_poll_interval: Duration::from_millis(
                crate::config::model::defaults::default_blocking_io_backoff_poll_interval_ms(),
            ),
        }
    }

    /// Avoid hardcoded timeout and retry values in runtime code paths.
    pub fn from_config(cfg: &Config) -> Self {
        Self::from_execution_tuning(&cfg.execution)
    }

    /// Some callers only have execution tuning and still need shared I/O policy behavior.
    pub fn from_execution_tuning(tuning: &ExecutionTuning) -> Self {
        Self {
            timeout: Duration::from_secs(tuning.copy_timeout_seconds.max(1)),
            retry_delays: tuning.retry_delays(),
            backoff_poll_interval: Duration::from_millis(
                tuning.blocking_io_backoff_poll_interval_ms.max(1),
            ),
        }
    }

    /// Early startup I/O still needs explicit timeout and retry behavior.
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

/// Let long-running blocking operations stop early during shutdown or cancellation.
#[derive(Clone, Copy, Debug, Default)]
pub struct CancellationFlag<'a> {
    flag: Option<&'a AtomicBool>,
}

impl<'a> CancellationFlag<'a> {
    /// Keep cancellation behavior explicit even when a caller does not support cancellation.
    pub fn none() -> Self {
        Self { flag: None }
    }

    /// Provide low-overhead cooperative cancellation for blocking paths.
    pub fn from_atomic(flag: &'a AtomicBool) -> Self {
        Self { flag: Some(flag) }
    }

    /// Keep cancellation checks consistent in one place.
    fn is_cancelled(&self) -> bool {
        self.flag
            .map(|f| f.load(Ordering::Relaxed))
            .unwrap_or(false)
    }
}

/// Ensure all blocking I/O paths use explicit and centralized resilience behavior.
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

/// Keep termination checks centralized and consistent.
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

/// Retry loops should not block shutdown or exceed time bounds while sleeping.
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
