//! # rustakka-agent-persona (stub)
//!
//! Persona = IQ + EQ + identity + style + values + goals + safety
//! rails. This crate is a **stub**; the real loader / validator /
//! `to_system_prompt` renderer is scheduled for Phase 2 (see
//! `../../docs/plan.md`).

use serde::{Deserialize, Serialize};

use rustakka_agent_eq::EqProfile;
use rustakka_agent_iq::IqProfile;
use rustakka_agent_traits::{Score, TraitSet};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Identity {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pronouns: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bio: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Register {
    #[default]
    Plain,
    Technical,
    Socratic,
    Casual,
    Formal,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CommunicationStyle {
    #[serde(default)]
    pub formality: Score,
    #[serde(default)]
    pub register: Register,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default)]
    pub signature_phrases: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MemoryPrefs {
    #[serde(default)]
    pub long_term: bool,
    #[serde(default)]
    pub summarize_after_turns: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SafetyRails {
    #[serde(default)]
    pub deny_topics: Vec<String>,
    #[serde(default)]
    pub refusal_style: Option<String>,
}

/// Aggregate persona bundle. All fields are optional/default-able, so
/// `Persona::default()` is a no-op overlay that leaves upstream
/// behavior unchanged.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Persona {
    #[serde(default)]
    pub identity: Identity,
    #[serde(default)]
    pub iq: IqProfile,
    #[serde(default)]
    pub eq: EqProfile,
    #[serde(default)]
    pub values: Vec<String>,
    #[serde(default)]
    pub goals: Vec<String>,
    #[serde(default)]
    pub style: CommunicationStyle,
    #[serde(default)]
    pub knowledge_domains: Vec<String>,
    #[serde(default)]
    pub taboos: Vec<String>,
    #[serde(default)]
    pub memory: MemoryPrefs,
    #[serde(default)]
    pub safety: SafetyRails,
    #[serde(default)]
    pub custom: TraitSet,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_persona_is_serde_roundtrippable() {
        let p = Persona::default();
        let s = serde_json::to_string(&p).unwrap();
        let back: Persona = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
    }
}
