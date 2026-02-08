/// Purpose: Legacy file-copy backup engine modules (feature-gated).
///
/// Inputs: Enabled via the `legacy-engine` cargo feature.
/// Outputs: Exposes the original scan-plan-execute pipeline when enabled.
/// Side effects: None.
/// Error handling: Delegated to module implementations.
/// Ties to other methods: Separate from the versioned engine (`backup::versioned`).
/// Why this exists: Keep historical engine code available for reference without confusing the default, versioned workflow.
#[cfg(feature = "legacy-engine")]
pub mod execution;
#[cfg(feature = "legacy-engine")]
pub mod naming;
#[cfg(feature = "legacy-engine")]
pub mod planning;
#[cfg(feature = "legacy-engine")]
pub mod retention;
pub mod versioned;
