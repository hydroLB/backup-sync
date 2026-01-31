use crate::state::models::{ActivityItem, StoredState};

/// Purpose: Records activity items and enforces a fixed capacity.
///
/// Inputs: the state to update, a new activity item, and the max history size.
/// Outputs: `()` after updating the state.
/// Ties to: backup execution and UI activity feeds.
/// Side effects: Mutates stored state by appending activity items.
/// Why: keep activity history bounded while retaining recent entries.
pub fn record_activity(state: &mut StoredState, item: ActivityItem, cap: usize) {
    state.recent_activity.push(item);
    trim_activity(&mut state.recent_activity, cap);
}

/// Purpose: Trims the activity list to the requested capacity.
///
/// Inputs: the activity list and maximum size.
/// Outputs: `()` after trimming.
/// Ties to: activity recording and memory management.
/// Side effects: Mutates the activity list by removing older entries.
/// Why: prevent unbounded growth of activity history.
pub fn trim_activity(activity: &mut Vec<ActivityItem>, cap: usize) {
    if cap == 0 {
        return;
    }
    if activity.len() > cap {
        let drop = activity.len() - cap;
        activity.drain(0..drop);
    }
}
