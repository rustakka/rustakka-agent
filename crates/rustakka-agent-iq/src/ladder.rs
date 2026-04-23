//! IQ tier / model-ladder (Phase 1b).
//!
//! An [`IqLadder`] binds a persona's [`super::IqProfile`] to a concrete
//! [`ChatModel`] at compile time, folding [`IqCarryings`] into the
//! caller's `CallOptions` along the way.
//!
//! ## Seam note
//!
//! The authoritative `ChatModel` type lives in
//! `rustakka-langgraph-providers`. Because `rustakka-agent-iq` must not
//! depend on the graph stack (see `docs/plan.md` § 5), we define a
//! local [`ChatModel`] trait that mirrors the minimal upstream surface
//! *exactly* — same method name (`model_name`), same shape — so the
//! `rustakka-agent-prebuilt` adapter only has to provide a blanket
//! `impl ChatModel for T where T: rustakka_langgraph_providers::ChatModel`.
//! Users in pure-data contexts can depend only on this crate.
//!
//! Likewise, [`CallOptionsLike`] mirrors the *writable* subset of
//! `rustakka_langgraph_providers::CallOptions`. `max_tokens` and
//! `temperature` are first-class; `top_p` is routed through `extra`
//! because that's where upstream keeps it.
//!
//! ## Fold order
//!
//! `IqCarryings` are composed deterministically:
//!
//! ```text
//! ladder.default  →  tier.default  →  rung  →  persona  →  caller
//! ```
//!
//! Later values win. See [`IqCarryings::fold_into`] for the primitive.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use rustakka_agent_traits::AgentEnv;

use super::IqProfile;

/// Coarse IQ bucket. Ranges are expressed against the composite score
/// produced by [`IqProfile::composite_score`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum IqTier {
    /// 0.00..0.20 — tiny, reactive agents (FAQ, classifier-style).
    Reflex,
    /// 0.20..0.40 — bounded tool loops, single-hop research.
    Operator,
    /// 0.40..0.60 — general assistant, 2–3 hop planning.
    Analyst,
    /// 0.60..0.80 — multi-step planning, self-critique, tool teams.
    Strategist,
    /// 0.80..1.00 — deep research, long-horizon, ensemble reasoning.
    Scholar,
}

impl IqTier {
    /// Bucket a composite score to a tier. Inclusive on the lower
    /// bound, exclusive on the upper, so `1.0 → Scholar`.
    pub fn from_score(score: f32) -> Self {
        let s = score.clamp(0.0, 1.0);
        if s < 0.20 {
            IqTier::Reflex
        } else if s < 0.40 {
            IqTier::Operator
        } else if s < 0.60 {
            IqTier::Analyst
        } else if s < 0.80 {
            IqTier::Strategist
        } else {
            IqTier::Scholar
        }
    }

    /// Next higher tier, or `None` if already at the top. Used by
    /// [`IqLadder::select`] when a ladder has a hole at this tier.
    pub fn upgrade(self) -> Option<IqTier> {
        match self {
            IqTier::Reflex => Some(IqTier::Operator),
            IqTier::Operator => Some(IqTier::Analyst),
            IqTier::Analyst => Some(IqTier::Strategist),
            IqTier::Strategist => Some(IqTier::Scholar),
            IqTier::Scholar => None,
        }
    }
}

/// Cache policy hint passed through to the provider layer.
///
/// Mirrors the upstream `rustakka-langgraph-providers::CachePolicy`
/// shape without depending on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CachePolicy {
    None,
    ShortLived,
    LongLived,
}

/// Call-time "carryings" — knobs applied to every LLM call made under
/// a given tier / rung / persona / caller.
///
/// Every field is optional; folding is "later wins" via
/// [`IqCarryings::fold_into`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct IqCarryings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Maps to `rustakka_langgraph_providers::CallOptions::max_tokens`.
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "max_output_tokens")]
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window_hint: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recursion_limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_policy: Option<CachePolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_allow_list: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt_addendum: Option<String>,
}

impl IqCarryings {
    /// Fold `self` into `dst`: for every field where `self` has
    /// `Some(_)`, overwrite `dst`. This implements "later wins".
    pub fn fold_into(&self, dst: &mut IqCarryings) {
        if let Some(v) = self.temperature {
            dst.temperature = Some(v);
        }
        if let Some(v) = self.top_p {
            dst.top_p = Some(v);
        }
        if let Some(v) = self.max_tokens {
            dst.max_tokens = Some(v);
        }
        if let Some(v) = self.context_window_hint {
            dst.context_window_hint = Some(v);
        }
        if let Some(v) = self.recursion_limit {
            dst.recursion_limit = Some(v);
        }
        if let Some(v) = self.cache_policy {
            dst.cache_policy = Some(v);
        }
        if let Some(v) = &self.tool_allow_list {
            dst.tool_allow_list = Some(v.clone());
        }
        if let Some(v) = &self.system_prompt_addendum {
            dst.system_prompt_addendum = Some(v.clone());
        }
    }

    /// Project the folded carryings onto a provider's `CallOptions`.
    ///
    /// `top_p` is routed via [`CallOptionsLike::set_extra`] under key
    /// `"top_p"` because that's where upstream's
    /// `rustakka_langgraph_providers::CallOptions` keeps it.
    pub fn apply_to<O: CallOptionsLike>(&self, opts: &mut O) {
        if let Some(t) = self.temperature {
            opts.set_temperature(t);
        }
        if let Some(p) = self.top_p {
            opts.set_extra("top_p", serde_json::json!(p));
        }
        if let Some(m) = self.max_tokens {
            opts.set_max_tokens(m);
        }
    }
}

/// Minimal, write-only mirror of the upstream
/// `rustakka_langgraph_providers::CallOptions` fields we care about.
///
/// `top_p` and any other "not first-class upstream" keys are routed via
/// [`set_extra`](Self::set_extra) so a blanket impl for the real
/// `CallOptions` in the adapter crate is one-line.
///
/// Model selection is *not* on this trait: upstream swaps models by
/// picking a different `Arc<dyn ChatModel>`, not via `CallOptions`.
/// The ladder returns both (rung-selected model + folded carryings),
/// and callers plug the model into their node factory.
pub trait CallOptionsLike {
    fn set_temperature(&mut self, v: f32);
    fn set_max_tokens(&mut self, v: u32);
    fn set_extra(&mut self, key: &str, value: serde_json::Value);
}

/// Minimal provider seam. The real `ChatModel` trait lives in
/// `rustakka-langgraph-providers`; [`rustakka-agent-prebuilt`] adapts
/// between the two. Method name matches upstream exactly so the
/// adapter's blanket impl is a one-liner.
pub trait ChatModel: std::fmt::Debug + Send + Sync + 'static {
    /// Stable identifier (e.g. `"gpt-4o"`, `"mock"`). Matches
    /// `rustakka_langgraph_providers::ChatModel::model_name`.
    fn model_name(&self) -> &str;

    /// `true` when this model is a deterministic mock, safe to use in
    /// snapshot tests.
    fn is_mock(&self) -> bool {
        false
    }
}

/// Deterministic mock model used when `AgentEnv::Test` is active.
#[derive(Debug, Clone, Copy)]
pub struct MockChatModel;

impl ChatModel for MockChatModel {
    fn model_name(&self) -> &str {
        "mock"
    }
    fn is_mock(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------
// Upstream adapters — opt-in via the `langgraph` feature so this crate
// stays graph-free for pure-data consumers.
// ---------------------------------------------------------------------

#[cfg(feature = "langgraph")]
mod langgraph_adapter {
    use super::{CallOptionsLike, ChatModel};
    use rustakka_langgraph_providers::prelude::{
        CallOptions as ProviderCallOptions, ChatModel as ProviderChatModel,
    };

    /// Blanket `CallOptionsLike` impl for upstream's `CallOptions`.
    /// `top_p` and any other non-first-class key are stored in
    /// `extra`, matching upstream's convention.
    impl CallOptionsLike for ProviderCallOptions {
        fn set_temperature(&mut self, v: f32) {
            self.temperature = Some(v);
        }
        fn set_max_tokens(&mut self, v: u32) {
            self.max_tokens = Some(v);
        }
        fn set_extra(&mut self, key: &str, value: serde_json::Value) {
            self.extra.insert(key.to_string(), value);
        }
    }

    /// Newtype that lifts an upstream `Arc<dyn ProviderChatModel>` to
    /// the agent-side `ChatModel` trait. The method name is identical
    /// upstream (`model_name`) so the forwarding is trivial.
    #[derive(Debug, Clone)]
    pub struct ProviderModel(pub std::sync::Arc<dyn ProviderChatModel>);

    impl ChatModel for ProviderModel {
        fn model_name(&self) -> &str {
            self.0.model_name()
        }
    }
}

#[cfg(feature = "langgraph")]
pub use langgraph_adapter::ProviderModel;

/// Predicate filter for a [`ModelRung`].
pub type RungPredicate = Arc<dyn Fn(&IqProfile) -> bool + Send + Sync>;

/// A single rung on a tier's model ladder.
#[derive(Clone)]
pub struct ModelRung {
    pub name: String,
    pub model: Arc<dyn ChatModel>,
    pub carryings: IqCarryings,
    /// Optional extra predicate: only pick this rung if it accepts the
    /// current [`IqProfile`]. When `None`, the rung always accepts.
    pub predicate: Option<RungPredicate>,
}

impl std::fmt::Debug for ModelRung {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelRung")
            .field("name", &self.name)
            .field("model", &self.model.model_name())
            .field("carryings", &self.carryings)
            .field("predicate", &self.predicate.as_ref().map(|_| "<fn>"))
            .finish()
    }
}

impl ModelRung {
    pub fn new(name: impl Into<String>, model: Arc<dyn ChatModel>) -> Self {
        Self {
            name: name.into(),
            model,
            carryings: IqCarryings::default(),
            predicate: None,
        }
    }

    pub fn with_carryings(mut self, c: IqCarryings) -> Self {
        self.carryings = c;
        self
    }

    pub fn with_predicate<F>(mut self, f: F) -> Self
    where
        F: Fn(&IqProfile) -> bool + Send + Sync + 'static,
    {
        self.predicate = Some(Arc::new(f));
        self
    }
}

/// Ordered ladder of model rungs for a single tier.
#[derive(Clone, Debug, Default)]
pub struct TierLadder {
    pub tier_default_carryings: IqCarryings,
    pub rungs: Vec<ModelRung>,
}

/// Full ladder across every tier.
///
/// Missing tiers fall back to the next-higher defined tier, then to
/// [`IqLadder::default_rung`].
#[derive(Clone, Debug, Default)]
pub struct IqLadder {
    pub default_carryings: IqCarryings,
    pub tiers: BTreeMap<IqTier, TierLadder>,
    pub default_rung: Option<ModelRung>,
}

impl IqLadder {
    pub fn builder() -> IqLadderBuilder {
        IqLadderBuilder::default()
    }

    /// Resolve a ladder rung for the profile.
    ///
    /// Selection order:
    /// 1. If `AgentEnv::Test` is active, return a [`MockChatModel`]
    ///    rung regardless of what the ladder says — this keeps
    ///    snapshot tests deterministic.
    /// 2. Probe the profile's tier, then higher tiers in order.
    /// 3. For each probed tier, walk rungs top-to-bottom and pick the
    ///    first one whose predicate (if any) accepts the profile.
    /// 4. Fall back to [`IqLadder::default_rung`].
    pub fn select(&self, profile: &IqProfile) -> Option<ModelRung> {
        if matches!(AgentEnv::current(), AgentEnv::Test) {
            return Some(ModelRung::new("mock", Arc::new(MockChatModel)));
        }

        let mut probe = Some(profile.tier());
        while let Some(tier) = probe {
            if let Some(ladder) = self.tiers.get(&tier) {
                for rung in &ladder.rungs {
                    let accepts = rung
                        .predicate
                        .as_ref()
                        .map(|p| p(profile))
                        .unwrap_or(true);
                    if accepts {
                        return Some(rung.clone());
                    }
                }
            }
            probe = tier.upgrade();
        }

        self.default_rung.clone()
    }

    /// Resolve the fully-folded [`IqCarryings`] for a profile:
    /// `ladder → tier → rung → profile-derived`.
    pub fn resolve_carryings(&self, profile: &IqProfile) -> IqCarryings {
        let mut out = IqCarryings::default();
        self.default_carryings.fold_into(&mut out);

        if let Some(ladder) = self.tiers.get(&profile.tier()) {
            ladder.tier_default_carryings.fold_into(&mut out);
        }

        if let Some(rung) = self.select(profile) {
            rung.carryings.fold_into(&mut out);
        }

        // Persona-derived overrides (from the IqProfile itself).
        let mut from_profile = IqCarryings::default();
        if let Some(t) = profile.temperature {
            from_profile.temperature = Some(t);
        }
        from_profile.fold_into(&mut out);

        out
    }

    /// Convenience: fold resolved carryings directly into a
    /// `CallOptions`-shaped target.
    ///
    /// Note: model selection is *not* folded into `CallOptions` — the
    /// ladder returns a rung (see [`IqLadder::select`]) whose
    /// `Arc<dyn ChatModel>` callers plug into their node factory.
    pub fn apply<O: CallOptionsLike>(&self, profile: &IqProfile, opts: &mut O) {
        let resolved = self.resolve_carryings(profile);
        resolved.apply_to(opts);
    }
}

/// Fluent builder for [`IqLadder`].
#[derive(Default, Debug, Clone)]
pub struct IqLadderBuilder {
    inner: IqLadder,
}

impl IqLadderBuilder {
    pub fn default_carryings(mut self, c: IqCarryings) -> Self {
        self.inner.default_carryings = c;
        self
    }

    pub fn default_rung(mut self, rung: ModelRung) -> Self {
        self.inner.default_rung = Some(rung);
        self
    }

    /// Append a rung to a tier, creating the tier ladder if needed.
    pub fn rung(mut self, tier: IqTier, rung: ModelRung) -> Self {
        self.inner
            .tiers
            .entry(tier)
            .or_default()
            .rungs
            .push(rung);
        self
    }

    /// Shorthand for a single-rung tier.
    pub fn tier(self, tier: IqTier, rung: ModelRung) -> Self {
        self.rung(tier, rung)
    }

    pub fn tier_default_carryings(mut self, tier: IqTier, c: IqCarryings) -> Self {
        self.inner
            .tiers
            .entry(tier)
            .or_default()
            .tier_default_carryings = c;
        self
    }

    pub fn build(self) -> IqLadder {
        self.inner
    }
}

// ---------------------------------------------------------------------
// YAML / TOML loaders.
//
// The external format is a stable subset of the ladder that does not
// carry live `Arc<dyn ChatModel>`s — only rung *names* and carryings.
// `IqLadder::bind_models` turns a [`IqLadderSpec`] into a real ladder
// by resolving rung names against a caller-supplied model registry.
// ---------------------------------------------------------------------

/// Serializable ladder specification. Compiled to [`IqLadder`] via
/// [`IqLadderSpec::bind`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct IqLadderSpec {
    #[serde(default)]
    pub default_carryings: IqCarryings,
    #[serde(default)]
    pub default_rung: Option<RungSpec>,
    #[serde(default)]
    pub tiers: BTreeMap<IqTier, TierLadderSpec>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TierLadderSpec {
    #[serde(default)]
    pub tier_default_carryings: IqCarryings,
    #[serde(default)]
    pub rungs: Vec<RungSpec>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RungSpec {
    /// Rung name; also used as the provider-model key.
    pub name: String,
    #[serde(default)]
    pub carryings: IqCarryings,
}

/// Error type for ladder loading.
#[derive(Debug, thiserror::Error)]
pub enum LadderError {
    #[error("rung `{0}` has no registered model")]
    UnknownModel(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl IqLadderSpec {
    pub fn from_json(s: &str) -> Result<Self, LadderError> {
        Ok(serde_json::from_str(s)?)
    }

    /// Bind rung names to concrete [`ChatModel`]s via a resolver
    /// closure. Returning `None` from the resolver yields
    /// [`LadderError::UnknownModel`].
    pub fn bind<F>(self, mut resolve: F) -> Result<IqLadder, LadderError>
    where
        F: FnMut(&str) -> Option<Arc<dyn ChatModel>>,
    {
        let mut ladder = IqLadder {
            default_carryings: self.default_carryings,
            default_rung: None,
            tiers: BTreeMap::new(),
        };

        let to_rung =
            |spec: RungSpec, resolve: &mut F| -> Result<ModelRung, LadderError> {
                let model = resolve(&spec.name)
                    .ok_or_else(|| LadderError::UnknownModel(spec.name.clone()))?;
                Ok(ModelRung {
                    name: spec.name,
                    model,
                    carryings: spec.carryings,
                    predicate: None,
                })
            };

        if let Some(d) = self.default_rung {
            ladder.default_rung = Some(to_rung(d, &mut resolve)?);
        }
        for (tier, spec) in self.tiers {
            let rungs: Result<Vec<_>, _> = spec
                .rungs
                .into_iter()
                .map(|r| to_rung(r, &mut resolve))
                .collect();
            ladder.tiers.insert(
                tier,
                TierLadder {
                    tier_default_carryings: spec.tier_default_carryings,
                    rungs: rungs?,
                },
            );
        }

        Ok(ladder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    // Tests that mutate `RUSTAKKA_AGENT_ENV` must be serialized; the
    // env is process-global.
    fn env_guard() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    #[derive(Debug, Default)]
    struct FakeOpts {
        temperature: Option<f32>,
        top_p: Option<f32>,
        max: Option<u32>,
    }
    impl CallOptionsLike for FakeOpts {
        fn set_temperature(&mut self, v: f32) {
            self.temperature = Some(v);
        }
        fn set_max_tokens(&mut self, v: u32) {
            self.max = Some(v);
        }
        fn set_extra(&mut self, key: &str, value: serde_json::Value) {
            if key == "top_p" {
                if let Some(p) = value.as_f64() {
                    self.top_p = Some(p as f32);
                }
            }
        }
    }

    #[derive(Debug)]
    struct NamedModel(&'static str);
    impl ChatModel for NamedModel {
        fn model_name(&self) -> &str {
            self.0
        }
    }

    #[test]
    fn tier_from_score_is_bucketed() {
        assert_eq!(IqTier::from_score(0.00), IqTier::Reflex);
        assert_eq!(IqTier::from_score(0.19), IqTier::Reflex);
        assert_eq!(IqTier::from_score(0.20), IqTier::Operator);
        assert_eq!(IqTier::from_score(0.40), IqTier::Analyst);
        assert_eq!(IqTier::from_score(0.60), IqTier::Strategist);
        assert_eq!(IqTier::from_score(0.80), IqTier::Scholar);
        assert_eq!(IqTier::from_score(1.0), IqTier::Scholar);
    }

    #[test]
    fn carryings_fold_order_is_later_wins() {
        let mut dst = IqCarryings::default();
        let a = IqCarryings {
            temperature: Some(0.1),
            max_tokens: Some(100),
            ..IqCarryings::default()
        };
        let b = IqCarryings {
            temperature: Some(0.7),
            ..IqCarryings::default()
        };
        a.fold_into(&mut dst);
        b.fold_into(&mut dst);
        assert_eq!(dst.temperature, Some(0.7));
        assert_eq!(dst.max_tokens, Some(100));
    }

    #[test]
    fn ladder_selects_rung_for_tier_then_falls_back_upwards() {
        let _g = env_guard();
        // Guarantee non-test env so ladder.select returns real rungs.
        std::env::set_var("RUSTAKKA_AGENT_ENV", "dev");
        let cheap = Arc::new(NamedModel("cheap"));
        let fancy = Arc::new(NamedModel("fancy"));
        let ladder = IqLadder::builder()
            .tier(IqTier::Operator, ModelRung::new("cheap", cheap))
            .tier(IqTier::Scholar, ModelRung::new("fancy", fancy))
            .build();

        let operator_profile = IqProfile::builder().pin_tier(IqTier::Operator).build();
        let analyst_profile = IqProfile::builder().pin_tier(IqTier::Analyst).build();
        let scholar_profile = IqProfile::builder().pin_tier(IqTier::Scholar).build();

        assert_eq!(ladder.select(&operator_profile).unwrap().name, "cheap");
        // Analyst has no rung defined; should fall back to next-higher
        // defined tier (Scholar).
        assert_eq!(ladder.select(&analyst_profile).unwrap().name, "fancy");
        assert_eq!(ladder.select(&scholar_profile).unwrap().name, "fancy");
        std::env::remove_var("RUSTAKKA_AGENT_ENV");
    }

    #[test]
    fn ladder_predicate_filters_rungs() {
        let _g = env_guard();
        std::env::set_var("RUSTAKKA_AGENT_ENV", "dev");
        let rung_a = ModelRung::new("a", Arc::new(NamedModel("a")))
            .with_predicate(|p| p.preferred_model.as_deref() == Some("a"));
        let rung_b = ModelRung::new("b", Arc::new(NamedModel("b")));
        let ladder = IqLadder::builder()
            .rung(IqTier::Analyst, rung_a)
            .rung(IqTier::Analyst, rung_b)
            .build();

        let want_a = IqProfile::builder()
            .pin_tier(IqTier::Analyst)
            .preferred_model("a")
            .build();
        let want_b = IqProfile::builder().pin_tier(IqTier::Analyst).build();

        assert_eq!(ladder.select(&want_a).unwrap().name, "a");
        assert_eq!(ladder.select(&want_b).unwrap().name, "b");
        std::env::remove_var("RUSTAKKA_AGENT_ENV");
    }

    #[test]
    fn ladder_forces_mock_when_agent_env_test() {
        let _g = env_guard();
        std::env::set_var("RUSTAKKA_AGENT_ENV", "test");
        let ladder = IqLadder::builder()
            .tier(
                IqTier::Scholar,
                ModelRung::new("fancy", Arc::new(NamedModel("fancy"))),
            )
            .build();
        let p = IqProfile::builder().pin_tier(IqTier::Scholar).build();
        let rung = ladder.select(&p).unwrap();
        assert_eq!(rung.name, "mock");
        assert!(rung.model.is_mock());
        std::env::remove_var("RUSTAKKA_AGENT_ENV");
    }

    #[test]
    fn ladder_applies_carryings_to_call_options() {
        let _g = env_guard();
        std::env::set_var("RUSTAKKA_AGENT_ENV", "dev");
        let rung = ModelRung::new("m", Arc::new(NamedModel("m"))).with_carryings(
            IqCarryings {
                temperature: Some(0.42),
                max_tokens: Some(777),
                ..IqCarryings::default()
            },
        );
        let ladder = IqLadder::builder().tier(IqTier::Analyst, rung).build();
        let profile = IqProfile::builder().pin_tier(IqTier::Analyst).build();
        let mut opts = FakeOpts::default();
        ladder.apply(&profile, &mut opts);
        assert_eq!(opts.temperature, Some(0.42));
        assert_eq!(opts.max, Some(777));
        // Model name now comes from the selected rung's model, not opts.
        assert_eq!(ladder.select(&profile).unwrap().model.model_name(), "m");
        std::env::remove_var("RUSTAKKA_AGENT_ENV");
    }

    #[test]
    fn ladder_spec_binds_via_resolver() {
        let json = r#"
            {
                "default_carryings": {"temperature": 0.2},
                "tiers": {
                    "Analyst": {
                        "rungs": [
                            {"name": "gpt-4o-mini", "carryings": {"max_tokens": 1024}}
                        ]
                    }
                }
            }
        "#;
        let spec = IqLadderSpec::from_json(json).unwrap();
        let ladder = spec
            .bind(|name| match name {
                "gpt-4o-mini" => Some(Arc::new(NamedModel("gpt-4o-mini")) as Arc<dyn ChatModel>),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            ladder.default_carryings.temperature,
            Some(0.2),
            "default carryings survive round-trip"
        );
        assert_eq!(ladder.tiers[&IqTier::Analyst].rungs.len(), 1);
    }

    #[test]
    fn ladder_spec_unknown_model_errors() {
        let spec = IqLadderSpec {
            tiers: [(
                IqTier::Reflex,
                TierLadderSpec {
                    rungs: vec![RungSpec {
                        name: "mystery".into(),
                        carryings: IqCarryings::default(),
                    }],
                    ..Default::default()
                },
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        };
        let err = spec.bind(|_| None).unwrap_err();
        assert!(matches!(err, LadderError::UnknownModel(ref n) if n == "mystery"));
    }
}
