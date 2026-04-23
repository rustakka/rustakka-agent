//! # rustakka-agent-eq (stub)
//!
//! Emotional characteristics for an agent. STUB for Phase 1 — see
//! `../../docs/plan.md`.

use serde::{Deserialize, Serialize};

use rustakka_agent_traits::{Score, TraitSet};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mood {
    #[default]
    Neutral,
    Upbeat,
    Calm,
    Serious,
    Playful,
    Stoic,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reflection {
    #[default]
    Never,
    AfterEachTurn,
    OnError,
    OnToolFailure,
}

/// Emotional profile for an agent.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EqProfile {
    #[serde(default)]
    pub empathy: Score,
    #[serde(default)]
    pub warmth: Score,
    #[serde(default)]
    pub assertiveness: Score,
    #[serde(default)]
    pub humor: Score,
    #[serde(default)]
    pub mood: Mood,
    #[serde(default)]
    pub reflection_cadence: Reflection,
    #[serde(default)]
    pub extra: TraitSet,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_is_serde_roundtrippable() {
        let p = EqProfile::default();
        let s = serde_json::to_string(&p).unwrap();
        let back: EqProfile = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
    }
}
