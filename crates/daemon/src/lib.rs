//! Daemon crate public API surface.
//!
//! External callers should use:
//! - `init` for startup argument and environment bootstrap contracts.
//! - `runtime` for daemon runtime entrypoints and IPC helpers.

pub mod init;
pub mod runtime;
