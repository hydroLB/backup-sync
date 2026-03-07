/// Summary: Original file-copy backup engine modules retained alongside the versioned engine.
///
/// Inputs: Module consumers in core and tests.
///
/// Outputs: Exposes the original scan-plan-execute pipeline.
///
/// Side effects: None.
///
/// Error handling: Delegated to module implementations.
///
/// Ties to other methods: Separate from the versioned engine (`backup::versioned`).
///
/// Why this exists: Preserve reusable planning and execution components while the versioned engine remains the production path.
pub mod execution;
pub mod naming;
pub mod planning;
pub mod retention;
pub mod versioned;
