use crate::state::models::{ActivityItem, StoredState};

/// Summary: Records activity items and enforces a fixed capacity.
///
/// Inputs: the state to update, a new activity item, and the max history size.
///
/// Outputs: `()` after updating the state.
///
/// Side effects: Mutates stored state by appending activity items.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: backup execution and UI activity feeds.
///
/// Why this exists: keep activity history bounded while retaining recent entries.
pub fn record_activity(state: &mut StoredState, item: ActivityItem, cap: usize) {
    state.recent_activity.push(item);
    trim_activity(&mut state.recent_activity, cap);
}

/// Summary: Trims the activity list to the requested capacity.
///
/// Inputs: the activity list and maximum size.
///
/// Outputs: `()` after trimming.
///
/// Side effects: Mutates the activity list by removing older entries.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: activity recording and memory management.
///
/// Why this exists: prevent unbounded growth of activity history.
pub fn trim_activity(activity: &mut Vec<ActivityItem>, cap: usize) {
    if cap == 0 {
        return;
    }
    if activity.len() > cap {
        let drop = activity.len() - cap;
        activity.drain(0..drop);
    }
}
