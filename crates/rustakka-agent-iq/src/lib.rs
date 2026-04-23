//! # rustakka-agent-iq (stub)
//!
//! Cognitive characteristics for an agent. This crate is a **stub**
//! introduced alongside the plan — the real implementation is
//! scheduled for Phase 1 (see `../../docs/plan.md`).
//!
//! The public shape defined here is the one targeted by Phase 1 so
//! that downstream crates can already import the types without
//! breaking changes later.

use serde::{Deserialize, Serialize};

use rustakka_agent_traits::{Score, TraitSet};

/// Cognitive profile for an agent.
///
/// Phase 1 will add:
/// - `apply_to_call_options(&self, &mut CallOptions)`
/// - `to_prompt_fragment(&self) -> Option<String>`
/// - `recommended_recursion_limit(&self) -> Option<u32>`
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct IqProfile {
    #[serde(default)]
    pub reasoning_depth: Score,

    #[serde(default)]
    pub planning_hops: u32,

    #[serde(default)]
    pub tool_eagerness: Score,

    #[serde(default)]
    pub verbosity: Score,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_model: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    #[serde(default)]
    pub extra: TraitSet,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_is_serde_roundtrippable() {
        let p = IqProfile::default();
        let s = serde_json::to_string(&p).unwrap();
        let back: IqProfile = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
    }
}
