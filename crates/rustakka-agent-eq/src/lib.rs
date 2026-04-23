//! # rustakka-agent-eq
//!
//! Emotional profile for an agent. An [`EqProfile`] is a small, typed
//! bundle of tone / reflection / mood knobs that feed:
//!
//! - a prompt fragment via [`EqProfile::to_prompt_fragment`],
//! - optional `reflect` node insertion via
//!   [`EqProfile::reflection_policy`].
//!
//! The profile is deliberately graph-free; graph integration lives in
//! `rustakka-agent-prebuilt`.

use serde::{Deserialize, Serialize};

use rustakka_agent_traits::{Score, TraitSet};

/// Canonical tone buckets.
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

impl Mood {
    /// Short, imperative guidance injected into prompt fragments.
    pub fn directive(self) -> &'static str {
        match self {
            Mood::Neutral => "Keep an even, professional tone.",
            Mood::Upbeat => "Be warm and encouraging without overstating.",
            Mood::Calm => "Stay measured and steady, especially under pressure.",
            Mood::Serious => "Be direct and sober; avoid levity.",
            Mood::Playful => "Allow light humor when it genuinely helps.",
            Mood::Stoic => "Minimize emotion; state facts plainly.",
        }
    }
}

/// When to insert a reflection step into the graph.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reflection {
    #[default]
    Never,
    /// After every `agent → tools` round-trip.
    AfterEachTurn,
    /// Only when the previous step raised an error.
    OnError,
    /// Only when a tool invocation failed.
    OnToolFailure,
}

/// Structured policy derived from [`EqProfile::reflection_cadence`]
/// that the graph builder can read without string matching.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReflectionPolicy {
    pub insert: bool,
    pub on_error: bool,
    pub on_tool_failure: bool,
    pub after_each_turn: bool,
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

impl EqProfile {
    pub fn builder() -> EqProfileBuilder {
        EqProfileBuilder::default()
    }

    pub fn reflection_policy(&self) -> ReflectionPolicy {
        match self.reflection_cadence {
            Reflection::Never => ReflectionPolicy::default(),
            Reflection::AfterEachTurn => ReflectionPolicy {
                insert: true,
                after_each_turn: true,
                ..ReflectionPolicy::default()
            },
            Reflection::OnError => ReflectionPolicy {
                insert: true,
                on_error: true,
                ..ReflectionPolicy::default()
            },
            Reflection::OnToolFailure => ReflectionPolicy {
                insert: true,
                on_tool_failure: true,
                ..ReflectionPolicy::default()
            },
        }
    }

    /// Deterministic prompt fragment describing the emotional stance.
    /// Returns `None` when the profile is entirely default.
    pub fn to_prompt_fragment(&self) -> Option<String> {
        let mut lines: Vec<String> = Vec::new();

        if self.empathy.get() > 0.0 {
            lines.push(format!(
                "Empathy: {:.2} — acknowledge the user's feelings before problem-solving.",
                self.empathy.get()
            ));
        }
        if self.warmth.get() > 0.0 {
            lines.push(format!(
                "Warmth: {:.2} — invite follow-ups; avoid cold phrasing.",
                self.warmth.get()
            ));
        }
        if self.assertiveness.get() > 0.0 {
            lines.push(format!(
                "Assertiveness: {:.2} — take a clear stance when asked for a recommendation.",
                self.assertiveness.get()
            ));
        }
        if self.humor.get() > 0.0 {
            lines.push(format!(
                "Humor: {:.2} — allow light, on-topic levity when it eases tension.",
                self.humor.get()
            ));
        }
        if self.mood != Mood::Neutral {
            lines.push(format!("Mood: {:?} — {}", self.mood, self.mood.directive()));
        } else {
            // Always emit the directive for Neutral too, when anything
            // else is present, so the fragment reads naturally.
            if !lines.is_empty() {
                lines.push(Mood::Neutral.directive().to_string());
            }
        }
        match self.reflection_cadence {
            Reflection::Never => {}
            Reflection::AfterEachTurn => {
                lines.push("Reflect briefly after every turn on what could improve.".into())
            }
            Reflection::OnError => {
                lines.push("When something goes wrong, pause and reflect before retrying.".into())
            }
            Reflection::OnToolFailure => lines
                .push("After a tool call fails, reflect on whether to retry or switch approach.".into()),
        }
        if let Some(extra) = self.extra.to_prompt_fragment() {
            lines.push(extra.trim_end().to_string());
        }

        if lines.is_empty() {
            None
        } else {
            Some(format!("Emotional stance:\n- {}", lines.join("\n- ")))
        }
    }
}

/// Typed builder for [`EqProfile`].
#[derive(Clone, Debug, Default)]
pub struct EqProfileBuilder {
    inner: EqProfile,
}

impl EqProfileBuilder {
    pub fn empathy(mut self, v: impl Into<Score>) -> Self {
        self.inner.empathy = v.into();
        self
    }
    pub fn warmth(mut self, v: impl Into<Score>) -> Self {
        self.inner.warmth = v.into();
        self
    }
    pub fn assertiveness(mut self, v: impl Into<Score>) -> Self {
        self.inner.assertiveness = v.into();
        self
    }
    pub fn humor(mut self, v: impl Into<Score>) -> Self {
        self.inner.humor = v.into();
        self
    }
    pub fn mood(mut self, m: Mood) -> Self {
        self.inner.mood = m;
        self
    }
    pub fn reflection(mut self, r: Reflection) -> Self {
        self.inner.reflection_cadence = r;
        self
    }
    pub fn extra(mut self, t: TraitSet) -> Self {
        self.inner.extra = t;
        self
    }
    pub fn build(self) -> EqProfile {
        self.inner
    }
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

    #[test]
    fn builder_sets_fields_and_policy() {
        let p = EqProfile::builder()
            .empathy(0.7)
            .warmth(0.6)
            .mood(Mood::Calm)
            .reflection(Reflection::OnError)
            .build();
        assert_eq!(p.mood, Mood::Calm);
        let policy = p.reflection_policy();
        assert!(policy.insert);
        assert!(policy.on_error);
        assert!(!policy.after_each_turn);
    }

    #[test]
    fn prompt_fragment_is_deterministic_and_stable() {
        let p = EqProfile::builder()
            .empathy(0.7)
            .warmth(0.5)
            .mood(Mood::Calm)
            .reflection(Reflection::AfterEachTurn)
            .build();
        let a = p.to_prompt_fragment().unwrap();
        let b = p.to_prompt_fragment().unwrap();
        assert_eq!(a, b);
        assert!(a.contains("Calm"));
        assert!(a.contains("Reflect briefly"));
        assert!(EqProfile::default().to_prompt_fragment().is_none());
    }
}
