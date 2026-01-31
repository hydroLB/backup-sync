pub mod debounce;
pub mod dirty;
pub mod watcher;

pub use dirty::DirtySet;
pub use watcher::{debounce_duration, start_watcher, take_dirty};
