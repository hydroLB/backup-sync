pub mod linux;
pub mod macos;
pub mod paths;
pub mod windows;

#[cfg(target_os = "linux")]
pub use linux as current;
#[cfg(target_os = "macos")]
pub use macos as current;
#[cfg(target_os = "windows")]
pub use windows as current;
