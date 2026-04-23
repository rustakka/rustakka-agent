//! # rustakka-agent-iq
//!
//! Cognitive profile for an agent. An [`IqProfile`] is a small, typed
//! bundle of knobs that a persona uses to shape LLM calls and graph
//! compilation:
//!
//! - [`reasoning_depth`](IqProfile::reasoning_depth) — chain-of-thought richness.
//! - [`planning_hops`](IqProfile::planning_hops) — rough hop budget; feeds
//!   `CompileConfig.recursion_limit` via [`recommended_recursion_limit`].
//! - [`tool_eagerness`](IqProfile::tool_eagerness) — router bias towards
//!   calling tools.
//! - [`verbosity`](IqProfile::verbosity) — controls brevity hints and
//!   `max_tokens` nudges.
//! - [`preferred_model`](IqProfile::preferred_model) /
//!   [`temperature`](IqProfile::temperature) — folded into provider
//!   `CallOptions` when a [`CallOptionsLike`] is bound.
//! - [`extra`](IqProfile::extra) — free-form user traits.
//!
//! The profile is deliberately dependency-free from graph internals so
//! it can be serialized, tested, and composed in non-graph contexts.
//!
//! The model ladder (tiers + rungs + carryings) lives in the
//! [`ladder`] submodule and is the bridge between an `IqProfile` and a
//! concrete [`ladder::ModelRung`] at compile/runtime.

use serde::{Deserialize, Serialize};

use rustakka_agent_traits::{Score, TraitSet};

pub mod ladder;

pub use ladder::{
    CachePolicy, CallOptionsLike, ChatModel, IqCarryings, IqLadder, IqLadderBuilder, IqTier,
    MockChatModel, ModelRung, TierLadder,
};

/// Cognitive profile for an agent.
///
/// Default is a quiet, low-verbosity "reflex" profile that leaves
/// upstream behavior unchanged — constructing `IqProfile::default()` is
/// safe in non-agent contexts (e.g. data validators).
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

    /// Optional explicit tier pin. When `None`, [`IqProfile::tier`]
    /// infers a tier from the composite cognitive score.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_tier: Option<IqTier>,
}

impl IqProfile {
    /// Start a builder with defaults.
    pub fn builder() -> IqProfileBuilder {
        IqProfileBuilder::default()
    }

    /// Composite cognitive score in `[0.0, 1.0]`, blending reasoning
    /// depth, (normalized) planning hops, and tool eagerness. Used by
    /// [`IqProfile::tier`] when no tier is pinned.
    pub fn composite_score(&self) -> f32 {
        // Weights chosen so depth dominates, planning is secondary,
        // and tool eagerness nudges the result. They sum to 1.0.
        let w_depth = 0.50_f32;
        let w_plan = 0.30_f32;
        let w_tool = 0.20_f32;

        // Normalize planning_hops to [0, 1] with a soft cap at 10 hops.
        let plan_norm = (self.planning_hops as f32 / 10.0).clamp(0.0, 1.0);

        (w_depth * self.reasoning_depth.get()
            + w_plan * plan_norm
            + w_tool * self.tool_eagerness.get())
        .clamp(0.0, 1.0)
    }

    /// Infer (or return the pinned) [`IqTier`].
    pub fn tier(&self) -> IqTier {
        if let Some(t) = self.pinned_tier {
            return t;
        }
        IqTier::from_score(self.composite_score())
    }

    /// Recommended recursion limit for compiled graphs. Maps
    /// `planning_hops` (plus a depth nudge) to a reasonable budget.
    /// Returns `None` when the profile has no opinion (defaulted).
    pub fn recommended_recursion_limit(&self) -> Option<u32> {
        if self.planning_hops == 0 && self.reasoning_depth.get() == 0.0 {
            return None;
        }
        let depth_nudge = (self.reasoning_depth.get() * 4.0).round() as u32;
        Some(self.planning_hops.saturating_mul(2).saturating_add(depth_nudge).max(4))
    }

    /// Emit a deterministic prompt fragment summarizing cognitive
    /// stance. Returns `None` when the profile is entirely default.
    pub fn to_prompt_fragment(&self) -> Option<String> {
        let mut lines: Vec<String> = Vec::new();

        if self.reasoning_depth.get() > 0.0 {
            lines.push(format!(
                "Reasoning depth: {:.2} (think step-by-step; surface your plan).",
                self.reasoning_depth.get()
            ));
        }
        if self.planning_hops > 0 {
            lines.push(format!(
                "Planning budget: up to {} hops before answering.",
                self.planning_hops
            ));
        }
        if self.tool_eagerness.get() > 0.0 {
            let bias = if self.tool_eagerness.get() >= 0.7 {
                "Prefer using tools whenever they could sharpen the answer."
            } else if self.tool_eagerness.get() >= 0.3 {
                "Use tools when they clearly help; otherwise answer directly."
            } else {
                "Only use tools when strictly required."
            };
            lines.push(bias.to_string());
        }
        if self.verbosity.get() > 0.0 {
            let target = ((1.0 - self.verbosity.get()) * 10.0 + 2.0).round() as u32;
            lines.push(format!(
                "Be concise: target ~{target} sentences per reply."
            ));
        }
        if let Some(extra) = self.extra.to_prompt_fragment() {
            lines.push(extra.trim_end().to_string());
        }

        if lines.is_empty() {
            None
        } else {
            Some(format!("Cognitive stance:\n- {}", lines.join("\n- ")))
        }
    }

    /// Fold IQ-driven adjustments into a provider's `CallOptions`.
    ///
    /// Only fields the profile has an opinion on are written, so
    /// callers can safely apply multiple profiles in order (later
    /// wins). Model selection is handled by the [`IqLadder`], *not*
    /// here, because upstream swaps models via
    /// `Arc<dyn ChatModel>` rather than a `CallOptions` field.
    pub fn apply_to_call_options<O: CallOptionsLike>(&self, opts: &mut O) {
        if let Some(t) = self.temperature {
            opts.set_temperature(t);
        }
        if self.verbosity.get() > 0.0 {
            // Soft cap on max_tokens based on verbosity.
            let cap = ((1.0 - self.verbosity.get()) * 3072.0 + 512.0).round() as u32;
            opts.set_max_tokens(cap);
        }
    }
}

/// Typed, chainable builder for [`IqProfile`].
#[derive(Clone, Debug, Default)]
pub struct IqProfileBuilder {
    inner: IqProfile,
}

impl IqProfileBuilder {
    pub fn reasoning_depth(mut self, v: impl Into<Score>) -> Self {
        self.inner.reasoning_depth = v.into();
        self
    }
    pub fn planning_hops(mut self, v: u32) -> Self {
        self.inner.planning_hops = v;
        self
    }
    pub fn tool_eagerness(mut self, v: impl Into<Score>) -> Self {
        self.inner.tool_eagerness = v.into();
        self
    }
    pub fn verbosity(mut self, v: impl Into<Score>) -> Self {
        self.inner.verbosity = v.into();
        self
    }
    pub fn preferred_model(mut self, m: impl Into<String>) -> Self {
        self.inner.preferred_model = Some(m.into());
        self
    }
    pub fn temperature(mut self, t: f32) -> Self {
        self.inner.temperature = Some(t);
        self
    }
    pub fn pin_tier(mut self, tier: IqTier) -> Self {
        self.inner.pinned_tier = Some(tier);
        self
    }
    pub fn extra(mut self, extra: TraitSet) -> Self {
        self.inner.extra = extra;
        self
    }
    pub fn build(self) -> IqProfile {
        self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustakka_agent_traits::{Dimension, Trait};

    #[test]
    fn default_profile_is_serde_roundtrippable() {
        let p = IqProfile::default();
        let s = serde_json::to_string(&p).unwrap();
        let back: IqProfile = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn builder_sets_fields() {
        let p = IqProfile::builder()
            .reasoning_depth(0.8)
            .planning_hops(5)
            .tool_eagerness(0.4)
            .verbosity(0.2)
            .preferred_model("gpt-4o")
            .temperature(0.3)
            .pin_tier(IqTier::Strategist)
            .build();
        assert_eq!(p.planning_hops, 5);
        assert_eq!(p.preferred_model.as_deref(), Some("gpt-4o"));
        assert_eq!(p.pinned_tier, Some(IqTier::Strategist));
        assert_eq!(p.tier(), IqTier::Strategist);
    }

    #[test]
    fn tier_inference_buckets_correctly() {
        let reflex = IqProfile::builder().reasoning_depth(0.0).build();
        let operator = IqProfile::builder().reasoning_depth(0.5).build();
        // Depth 0.7 w=0.5 → 0.35 (Operator band). Need composite >= 0.4 for Analyst.
        let analyst = IqProfile::builder()
            .reasoning_depth(0.7)
            .planning_hops(3)
            .tool_eagerness(0.2)
            .build();
        let strategist = IqProfile::builder()
            .reasoning_depth(0.8)
            .planning_hops(6)
            .tool_eagerness(0.6)
            .build();
        let scholar = IqProfile::builder()
            .reasoning_depth(1.0)
            .planning_hops(10)
            .tool_eagerness(1.0)
            .build();
        assert_eq!(reflex.tier(), IqTier::Reflex);
        assert_eq!(operator.tier(), IqTier::Operator);
        assert_eq!(analyst.tier(), IqTier::Analyst);
        assert_eq!(strategist.tier(), IqTier::Strategist);
        assert_eq!(scholar.tier(), IqTier::Scholar);
    }

    #[test]
    fn prompt_fragment_is_deterministic_and_omits_defaults() {
        let p = IqProfile::builder()
            .reasoning_depth(0.7)
            .planning_hops(3)
            .tool_eagerness(0.9)
            .verbosity(0.5)
            .extra(
                TraitSet::new().with(Trait::new("curiosity", 0.8, Dimension::Iq)),
            )
            .build();
        let a = p.to_prompt_fragment().unwrap();
        let b = p.to_prompt_fragment().unwrap();
        assert_eq!(a, b);
        assert!(a.contains("Reasoning depth"));
        assert!(a.contains("curiosity"));
        assert!(IqProfile::default().to_prompt_fragment().is_none());
    }

    #[test]
    fn recommended_recursion_limit_uses_depth_and_hops() {
        let p = IqProfile::builder()
            .planning_hops(3)
            .reasoning_depth(0.5)
            .build();
        let rl = p.recommended_recursion_limit().unwrap();
        assert!(rl >= 4);
        assert!(IqProfile::default().recommended_recursion_limit().is_none());
    }
}
