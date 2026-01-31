use crate::backup::planning::BackupPlan;
use crate::config::model::PlanningTuning;
use anyhow::Result;

/// Purpose: Enforces plan guardrails to prevent oversized backup runs.
///
/// Inputs: the computed plan and planning tuning values.
/// Outputs: `Ok(())` when the plan is within configured limits.
/// Ties to: planning and config tuning for plan size limits.
/// Side effects: None.
/// Why: keep backup cycles bounded to protect IO and runtime latency.
pub fn enforce_plan_limits(plan: &BackupPlan, tuning: &PlanningTuning) -> Result<()> {
    if plan.len() > tuning.max_plan_items {
        anyhow::bail!(
            "planning::enforce_plan_limits too many items to back up in one run (>{}); refine watched paths or add ignores",
            tuning.max_plan_items
        );
    }
    Ok(())
}
