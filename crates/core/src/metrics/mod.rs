use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct HistogramState {
    count: u64,
    sum_ms: u64,
    max_ms: u64,
    last_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MetricsSnapshot {
    pub counters: BTreeMap<&'static str, u64>,
    pub histograms: BTreeMap<&'static str, (u64, u64, u64, u64)>,
}

fn counters() -> &'static Mutex<BTreeMap<&'static str, u64>> {
    static COUNTERS: OnceLock<Mutex<BTreeMap<&'static str, u64>>> = OnceLock::new();
    COUNTERS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn histograms() -> &'static Mutex<BTreeMap<&'static str, HistogramState>> {
    static HISTOGRAMS: OnceLock<Mutex<BTreeMap<&'static str, HistogramState>>> = OnceLock::new();
    HISTOGRAMS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Provide a zero-dependency metric hook surface for production wiring.
pub fn counter_inc(name: &'static str, by: u64) {
    if by == 0 {
        return;
    }
    let mut guard = counters().lock().expect("metrics::counter_inc poisoned");
    *guard.entry(name).or_insert(0) = guard.get(name).copied().unwrap_or(0).saturating_add(by);
}

/// Keep time-series hooks available without requiring an external metrics backend.
pub fn observe_duration(name: &'static str, elapsed: Duration) {
    let elapsed_ms = elapsed.as_millis().min(u64::MAX as u128) as u64;
    let mut guard = histograms()
        .lock()
        .expect("metrics::observe_duration poisoned");
    let state = guard.entry(name).or_default();
    state.count = state.count.saturating_add(1);
    state.sum_ms = state.sum_ms.saturating_add(elapsed_ms);
    state.max_ms = state.max_ms.max(elapsed_ms);
    state.last_ms = elapsed_ms;
}

/// Expose stable metric hook output for validation and plumbing.
pub fn snapshot() -> MetricsSnapshot {
    let counters = counters()
        .lock()
        .expect("metrics::snapshot counters lock poisoned")
        .clone();
    let histograms = histograms()
        .lock()
        .expect("metrics::snapshot histograms lock poisoned")
        .iter()
        .map(|(name, state)| {
            (
                *name,
                (state.count, state.sum_ms, state.max_ms, state.last_ms),
            )
        })
        .collect();
    MetricsSnapshot {
        counters,
        histograms,
    }
}

#[cfg(test)]
pub fn reset_for_tests() {
    counters()
        .lock()
        .expect("metrics::reset_for_tests counters lock poisoned")
        .clear();
    histograms()
        .lock()
        .expect("metrics::reset_for_tests histograms lock poisoned")
        .clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_inc_updates_snapshot() {
        reset_for_tests();
        counter_inc("ipc_requests_total", 1);
        counter_inc("ipc_requests_total", 2);
        let snap = snapshot();
        assert_eq!(snap.counters.get("ipc_requests_total"), Some(&3));
    }

    #[test]
    fn observe_duration_updates_histogram_state() {
        reset_for_tests();
        observe_duration("ipc_latency_ms", Duration::from_millis(10));
        observe_duration("ipc_latency_ms", Duration::from_millis(25));
        let snap = snapshot();
        assert_eq!(
            snap.histograms.get("ipc_latency_ms"),
            Some(&(2, 35, 25, 25))
        );
    }
}
