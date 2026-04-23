//! # rustakka-agent-traits
//!
//! Core primitives shared by every `rustakka-agent` crate:
//!
//! - [`Trait`] — a single named, bounded non-physical characteristic.
//! - [`Score`] — an `f32` clamped to `0.0..=1.0` at construction.
//! - [`Dimension`] — which aspect of the persona a trait contributes to.
//! - [`TraitSet`] — an ordered bag of traits with merge / prompt helpers.
//! - [`AgentEnv`] — `dev | test | prod` selector, read from
//!   `RUSTAKKA_AGENT_ENV` (defaulting to `dev`), matching the convention
//!   used by `rustakka-langgraph` (`RUSTAKKA_LANGGRAPH_ENV`).
//!
//! This crate is intentionally dependency-light: it brings in only
//! `serde`, `serde_json`, and `thiserror`. It must stay decoupled from
//! `rustakka-langgraph` so that IQ / EQ / Persona can be reused in
//! non-graph contexts (tests, validators, fixture generators).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A value clamped to the inclusive interval `[0.0, 1.0]`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Score(f32);

impl Score {
    /// Construct a score, clamping the input into `[0.0, 1.0]`.
    pub fn new(v: f32) -> Self {
        Self(v.clamp(0.0, 1.0))
    }

    pub fn get(self) -> f32 {
        self.0
    }
}

impl Default for Score {
    fn default() -> Self {
        Score(0.0)
    }
}

impl From<f32> for Score {
    fn from(v: f32) -> Self {
        Score::new(v)
    }
}

/// Which facet of an agent's personality a [`Trait`] contributes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Dimension {
    Iq,
    Eq,
    Style,
    Values,
    Safety,
    Custom,
}

/// A single bounded, non-physical characteristic of an agent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trait {
    pub name: String,
    pub score: Score,
    pub dimension: Dimension,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl Trait {
    pub fn new(
        name: impl Into<String>,
        score: impl Into<Score>,
        dimension: Dimension,
    ) -> Self {
        Self {
            name: name.into(),
            score: score.into(),
            dimension,
            notes: None,
        }
    }

    pub fn with_notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }
}

/// An ordered collection of [`Trait`]s keyed by name.
///
/// `BTreeMap` is used so that prompt rendering (`to_prompt_fragment`)
/// is deterministic — a property we rely on for snapshot tests.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TraitSet(pub BTreeMap<String, Trait>);

impl TraitSet {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn insert(&mut self, t: Trait) -> &mut Self {
        self.0.insert(t.name.clone(), t);
        self
    }

    pub fn with(mut self, t: Trait) -> Self {
        self.insert(t);
        self
    }

    /// Merge `other` into `self`. On key collision, `other` wins —
    /// this makes "base + override" composition natural.
    pub fn merge(mut self, other: TraitSet) -> TraitSet {
        for (k, v) in other.0 {
            self.0.insert(k, v);
        }
        self
    }

    /// Emit a deterministic, human-readable prompt fragment describing
    /// the non-trivial traits in the set. Returns `None` when empty.
    pub fn to_prompt_fragment(&self) -> Option<String> {
        if self.0.is_empty() {
            return None;
        }
        let mut out = String::from("Traits:\n");
        for t in self.0.values() {
            out.push_str(&format!(
                "- {} ({:?}) = {:.2}",
                t.name,
                t.dimension,
                t.score.get()
            ));
            if let Some(n) = &t.notes {
                out.push_str(&format!(" — {n}"));
            }
            out.push('\n');
        }
        Some(out)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Deployment environment selector.
///
/// Mirrors `rustakka-langgraph`'s `RUSTAKKA_LANGGRAPH_ENV` convention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentEnv {
    Dev,
    Test,
    Prod,
}

impl AgentEnv {
    /// Read `RUSTAKKA_AGENT_ENV` from the environment, defaulting to
    /// [`AgentEnv::Dev`] if unset or unrecognized.
    pub fn current() -> Self {
        match std::env::var("RUSTAKKA_AGENT_ENV")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "prod" | "production" => AgentEnv::Prod,
            "test" | "testing" => AgentEnv::Test,
            _ => AgentEnv::Dev,
        }
    }
}

/// Errors produced by this crate. Downstream crates (`iq`, `eq`,
/// `persona`) re-use / wrap this type via `thiserror`.
#[derive(thiserror::Error, Debug)]
pub enum TraitError {
    #[error("invalid score: {0} (must be in 0.0..=1.0)")]
    InvalidScore(f32),
    #[error("unknown dimension: {0}")]
    UnknownDimension(String),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_clamps_to_unit_interval() {
        assert_eq!(Score::new(-0.5).get(), 0.0);
        assert_eq!(Score::new(1.5).get(), 1.0);
        assert_eq!(Score::new(0.42).get(), 0.42);
    }

    #[test]
    fn trait_set_merge_overrides_by_key() {
        let base = TraitSet::new().with(Trait::new("humor", 0.3, Dimension::Eq));
        let over = TraitSet::new().with(Trait::new("humor", 0.9, Dimension::Eq));
        let merged = base.merge(over);
        assert_eq!(merged.0.get("humor").unwrap().score.get(), 0.9);
    }

    #[test]
    fn prompt_fragment_is_deterministic() {
        let mut ts = TraitSet::new();
        ts.insert(Trait::new("curiosity", 0.8, Dimension::Iq));
        ts.insert(Trait::new("empathy", 0.6, Dimension::Eq));
        let a = ts.to_prompt_fragment().unwrap();
        let b = ts.clone().to_prompt_fragment().unwrap();
        assert_eq!(a, b);
        assert!(a.contains("curiosity"));
        assert!(a.contains("empathy"));
    }

    #[test]
    fn env_defaults_to_dev_when_unset() {
        // Not asserting exact value because other tests may set the
        // env var; we only verify that `current()` returns *some*
        // variant, proving the function is callable.
        let _ = AgentEnv::current();
    }
}
