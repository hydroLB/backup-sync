use crate::backup::planning::BackupPlan;
use crate::config::model::PlanningTuning;
use anyhow::Result;

/// Summary: Enforces plan guardrails to prevent oversized backup runs.
///
/// Inputs: the computed plan and planning tuning values.
///
/// Outputs: `Ok(())` when the plan is within configured limits.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: planning and config tuning for plan size limits.
///
/// Why this exists: keep backup cycles bounded to protect IO and runtime latency.
pub fn enforce_plan_limits(plan: &BackupPlan, tuning: &PlanningTuning) -> Result<()> {
    if plan.len() > tuning.max_plan_items {
        anyhow::bail!(
            "planning::enforce_plan_limits too many items to back up in one run (>{}); refine watched paths or add ignores",
            tuning.max_plan_items
        );
    }
    Ok(())
}
