use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanningTuning {
    #[serde(default = "crate::config::model::defaults::default_hash_check_interval")]
    pub hash_check_interval: u32,
    #[serde(default = "crate::config::model::defaults::default_max_plan_items")]
    pub max_plan_items: usize,
    #[serde(default = "crate::config::model::defaults::default_scan_timeout_seconds")]
    pub scan_timeout_seconds: u64,
    #[serde(default = "crate::config::model::defaults::default_scan_capacity_multiplier")]
    pub scan_capacity_multiplier: usize,
}

impl Default for PlanningTuning {
    /// Purpose: Builds a baseline planning tuning profile for hashing cadence and plan caps.
    ///
    /// Inputs: the default functions in `config::model::defaults`.
    /// Outputs: a fully populated tuning profile.
    /// Ties to: backup planning and guardrail enforcement.
    /// Side effects: None.
    /// Why: centralize planning knobs so defaults stay aligned across the codebase.
    fn default() -> Self {
        Self {
            hash_check_interval: crate::config::model::defaults::default_hash_check_interval(),
            max_plan_items: crate::config::model::defaults::default_max_plan_items(),
            scan_timeout_seconds: crate::config::model::defaults::default_scan_timeout_seconds(),
            scan_capacity_multiplier:
                crate::config::model::defaults::default_scan_capacity_multiplier(),
        }
    }
}
