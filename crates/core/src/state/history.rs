use crate::state::models::{ActivityItem, StoredState};

/// Keep activity history bounded while retaining recent entries.
pub fn record_activity(state: &mut StoredState, item: ActivityItem, cap: usize) {
    state.recent_activity.push(item);
    trim_activity(&mut state.recent_activity, cap);
}

/// Prevent unbounded growth of activity history.
pub fn trim_activity(activity: &mut Vec<ActivityItem>, cap: usize) {
    if cap == 0 {
        return;
    }
    if activity.len() > cap {
        let drop = activity.len() - cap;
        activity.drain(0..drop);
    }
}
