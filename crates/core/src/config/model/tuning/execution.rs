use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::warn;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTuning {
    #[serde(default = "crate::config::model::defaults::default_copy_buffer_bytes")]
    pub copy_buffer_bytes: usize,
    #[serde(default = "crate::config::model::defaults::default_copy_timeout_seconds")]
    pub copy_timeout_seconds: u64,
    #[serde(default = "crate::config::model::defaults::default_free_space_safety_buffer_bytes")]
    pub free_space_safety_buffer_bytes: u64,
    #[serde(default = "crate::config::model::defaults::default_recent_activity_cap")]
    pub recent_activity_cap: usize,
    #[serde(default = "crate::config::model::defaults::default_retry_delays_ms")]
    pub retry_delays_ms: Vec<u64>,
    #[serde(default = "crate::config::model::defaults::default_retry_jitter_pct")]
    pub retry_jitter_pct: f64,
    #[serde(
        default = "crate::config::model::defaults::default_blocking_io_backoff_poll_interval_ms"
    )]
    pub blocking_io_backoff_poll_interval_ms: u64,
}

impl Default for ExecutionTuning {
    /// Summary: Builds a baseline execution tuning profile for copy, safety, and retry behavior.
    ///
    /// Inputs: the default functions in `config::model::defaults`.
    ///
    /// Outputs: a fully populated tuning profile.
    ///
    /// Side effects: None.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: `BackupExecutor` for copy buffering, free space checks, and retry backoff.
    ///
    /// Why this exists: centralize execution knobs so defaults stay aligned across the codebase.
    fn default() -> Self {
        Self {
            copy_buffer_bytes: crate::config::model::defaults::default_copy_buffer_bytes(),
            copy_timeout_seconds: crate::config::model::defaults::default_copy_timeout_seconds(),
            free_space_safety_buffer_bytes:
                crate::config::model::defaults::default_free_space_safety_buffer_bytes(),
            recent_activity_cap: crate::config::model::defaults::default_recent_activity_cap(),
            retry_delays_ms: crate::config::model::defaults::default_retry_delays_ms(),
            retry_jitter_pct: crate::config::model::defaults::default_retry_jitter_pct(),
            blocking_io_backoff_poll_interval_ms:
                crate::config::model::defaults::default_blocking_io_backoff_poll_interval_ms(),
        }
    }
}

impl ExecutionTuning {
    /// Summary: Converts configured millisecond delays into `Duration` values.
    ///
    /// Inputs: the configured list of delay milliseconds.
    ///
    /// Outputs: a vector of `Duration` values in the same order.
    ///
    /// Side effects: Reads system time for jitter entropy.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: `BackupExecutor::execute` for retry scheduling.
    ///
    /// Why this exists: avoid duplicating conversion logic across execution paths.
    pub(crate) fn retry_delays(&self) -> Vec<Duration> {
        self.retry_delays_ms
            .iter()
            .map(|ms| {
                let base = Duration::from_millis(*ms);
                jitter_duration(base, self.retry_jitter_pct)
            })
            .collect()
    }
}

/// Summary: Applies jitter to a base duration using a percentage window.
///
/// Inputs: the base delay and jitter percentage.
///
/// Outputs: a jittered delay duration.
///
/// Side effects: Reads system time for jitter entropy.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `ExecutionTuning::retry_delays` for retry scheduling variance.
///
/// Why this exists: reduce coordinated retries while preserving average backoff.
fn jitter_duration(delay: Duration, jitter_pct: f64) -> Duration {
    if jitter_pct <= 0.0 {
        return delay;
    }
    let pct = jitter_pct.clamp(0.0, 1.0);
    let nanos = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(delta) => delta.subsec_nanos(),
        Err(e) => {
            warn!(
                "config::model::tuning::execution::jitter_duration clock skew detected, using zero jitter: {}",
                e
            );
            0
        }
    };
    let rand = (nanos % 1000) as f64 / 1000.0;
    let factor = 1.0 - pct + (2.0 * pct * rand);
    Duration::from_secs_f64(delay.as_secs_f64() * factor)
}
