//! AI-Q-style deep-research graph (Phase 5a).
//!
//! Modeled after
//! [NVIDIA's AI-Q Research Agent Blueprint](https://docs.nvidia.com/aiq-blueprint/2.0.0/architecture/overview.html) —
//! orchestrator state machine, intent classifier, shallow/deep split,
//! planner → researcher → critic loop, citation verification.
//!
//! See `docs/aiq-research.md` for the authoritative topology.

use std::sync::Arc;

use rustakka_agent_iq::{IqLadder, IqTier};
use rustakka_agent_persona::Persona;

use crate::graph::{AgentGraph, Blueprint, ChannelKind, ChannelSpec, GraphResult};

pub mod channels {
    pub const MESSAGES: &str = "messages";
    pub const INTENT: &str = "aiq.intent";
    pub const PLAN: &str = "aiq.plan";
    pub const EVIDENCE: &str = "aiq.evidence";
    pub const CRITIQUES: &str = "aiq.critiques";
    pub const CITATIONS: &str = "aiq.citations";
    pub const REPORT: &str = "aiq.report";
    pub const SANITIZATION_LOG: &str = "aiq.sanitization";
}

/// Trait for verifying citations. Default impl is a no-op; the test-
/// mode fixture impl returns deterministic "verified" marks so
/// snapshot tests are stable.
pub trait CitationVerifier: Send + Sync + std::fmt::Debug {
    fn verify(&self, citation: &str) -> bool;
}

#[derive(Debug)]
pub struct DefaultCitationVerifier;

impl CitationVerifier for DefaultCitationVerifier {
    fn verify(&self, _citation: &str) -> bool {
        true
    }
}

#[derive(Debug)]
pub struct FixtureCitationVerifier;

impl CitationVerifier for FixtureCitationVerifier {
    fn verify(&self, citation: &str) -> bool {
        !citation.is_empty()
    }
}

/// Trait for sanitizing the final report before returning it.
pub trait ReportSanitizer: Send + Sync + std::fmt::Debug {
    fn sanitize(&self, report: &str) -> String;
}

#[derive(Debug)]
pub struct DefaultReportSanitizer;

impl ReportSanitizer for DefaultReportSanitizer {
    fn sanitize(&self, report: &str) -> String {
        report.trim().to_string()
    }
}

/// Ensemble configuration: run the deep path N times in parallel and
/// reconcile.
#[derive(Clone, Debug, Default)]
pub struct EnsembleConfig {
    pub parallel_runs: u32,
}

/// Tools available to the research graph. Free-form names because
/// real tool bindings live in the upstream engine adapter.
#[derive(Clone, Debug, Default)]
pub struct AiqToolkit {
    pub search: Vec<String>,
    pub retriever: Vec<String>,
    pub code: Vec<String>,
}

/// Options for [`create_aiq_research_agent`].
#[derive(Clone, Debug)]
pub struct AiqResearchOptions {
    pub persona: Option<Persona>,
    pub ladder: IqLadder,
    pub allow_deep_path: bool,
    pub hitl_clarifier: bool,
    pub ensemble: Option<EnsembleConfig>,
    pub post_hoc_refiner: bool,
    pub citation_verifier: Arc<dyn CitationVerifier>,
    pub sanitizer: Arc<dyn ReportSanitizer>,
    pub tools: AiqToolkit,
}

impl Default for AiqResearchOptions {
    fn default() -> Self {
        Self {
            persona: None,
            ladder: IqLadder::default(),
            allow_deep_path: true,
            hitl_clarifier: false,
            ensemble: None,
            post_hoc_refiner: true,
            citation_verifier: Arc::new(DefaultCitationVerifier),
            sanitizer: Arc::new(DefaultReportSanitizer),
            tools: AiqToolkit::default(),
        }
    }
}

/// Default per-subagent IQ-tier mapping (see `docs/plan.md` § 5a.1).
pub fn default_subagent_tiers() -> Vec<(&'static str, IqTier)> {
    vec![
        ("clarifier", IqTier::Operator),
        ("intent_classifier", IqTier::Reflex),
        ("shallow_researcher", IqTier::Analyst),
        ("planner", IqTier::Strategist),
        ("researcher", IqTier::Strategist),
        ("evidence_gatherer", IqTier::Analyst),
        ("comparator", IqTier::Analyst),
        ("critic", IqTier::Strategist),
        ("synthesizer", IqTier::Scholar),
        ("post_hoc_refiner", IqTier::Strategist),
    ]
}

fn channel_specs() -> Vec<ChannelSpec> {
    vec![
        ChannelSpec::messages(channels::MESSAGES),
        ChannelSpec::last(channels::INTENT),
        ChannelSpec::last(channels::PLAN),
        ChannelSpec::appended(channels::EVIDENCE),
        ChannelSpec::appended(channels::CRITIQUES),
        ChannelSpec::appended(channels::CITATIONS),
        ChannelSpec::last(channels::REPORT),
        ChannelSpec::appended(channels::SANITIZATION_LOG),
    ]
}

/// Build the AI-Q deep-research [`Blueprint`]. For the runnable
/// upstream graph, call [`Blueprint::compile`](crate::graph::Blueprint::compile)
/// on the result, or use [`create_aiq_research_agent`] for a
/// pre-compiled [`AgentGraph`].
pub fn build_aiq_research_blueprint(opts: &AiqResearchOptions) -> Blueprint {
    let mut g = Blueprint::new("aiq_research");
    g.add_node("start");
    g.add_node("end");
    g.add_node("clarifier");
    g.add_node("intent_classifier");
    g.add_node("shallow_researcher");
    g.add_node("planner");
    g.add_node("researcher");
    g.add_node("evidence_gatherer");
    g.add_node("comparator");
    g.add_node("critic");
    g.add_node("synthesizer");
    g.add_node("citation_verifier");

    g.add_edge("start", "clarifier");
    g.add_edge("clarifier", "intent_classifier");
    g.add_edge("intent_classifier", "shallow_researcher");
    g.add_edge("shallow_researcher", "citation_verifier");

    if opts.allow_deep_path {
        g.add_edge("intent_classifier", "planner");
        g.add_edge("planner", "researcher");
        for leaf in ["evidence_gatherer", "comparator", "critic"] {
            g.add_edge("researcher", leaf);
            g.add_edge(leaf, "synthesizer");
        }
        g.add_edge("synthesizer", "citation_verifier");
    }

    if opts.post_hoc_refiner {
        g.add_node("post_hoc_refiner");
        g.add_edge("citation_verifier", "post_hoc_refiner");
        g.add_edge("post_hoc_refiner", "end");
    } else {
        g.add_edge("citation_verifier", "end");
    }

    if opts.hitl_clarifier {
        g.interrupt_before.push("clarifier".into());
    }

    for spec in channel_specs() {
        g.channels.insert(spec.name, spec.kind);
    }
    if let Some(e) = &opts.ensemble {
        g.channels.insert(
            format!("aiq.ensemble.runs.{}", e.parallel_runs),
            ChannelKind::LastValue,
        );
    }
    if let Some(p) = &opts.persona {
        g.system_prompt = Some(p.to_system_prompt()).filter(|s| !s.is_empty());
    }
    g
}

/// Compile the AI-Q research graph into a real upstream
/// [`CompiledStateGraph`](crate::graph::CompiledStateGraph), returned
/// as an [`AgentGraph`] so callers can also introspect the blueprint.
pub async fn create_aiq_research_agent(
    opts: AiqResearchOptions,
) -> GraphResult<AgentGraph> {
    let blueprint = build_aiq_research_blueprint(&opts);
    let compiled = blueprint.compile().await?;
    Ok(AgentGraph {
        blueprint,
        call_options: Default::default(),
        tools: Vec::new(),
        model: None,
        compiled: std::sync::Arc::new(compiled),
        store: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shallow_path_only_reaches_shallow_researcher() {
        let g = create_aiq_research_agent(AiqResearchOptions {
            allow_deep_path: false,
            ..Default::default()
        })
        .await
        .unwrap();
        assert!(g.blueprint.has_edge("intent_classifier", "shallow_researcher"));
        assert!(!g.blueprint.has_edge("intent_classifier", "planner"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn deep_path_fan_out_three_ways() {
        let g = create_aiq_research_agent(AiqResearchOptions::default())
            .await
            .unwrap();
        for leaf in ["evidence_gatherer", "comparator", "critic"] {
            assert!(
                g.blueprint.has_edge("researcher", leaf),
                "missing researcher → {leaf}"
            );
            assert!(
                g.blueprint.has_edge(leaf, "synthesizer"),
                "missing {leaf} → synthesizer"
            );
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn hitl_clarifier_registers_interrupt() {
        let g = create_aiq_research_agent(AiqResearchOptions {
            hitl_clarifier: true,
            ..Default::default()
        })
        .await
        .unwrap();
        assert!(g
            .blueprint
            .interrupt_before
            .contains(&"clarifier".to_string()));
    }

    #[test]
    fn default_subagent_tiers_contain_synthesizer() {
        let tiers = default_subagent_tiers();
        assert!(tiers.iter().any(|(n, t)| *n == "synthesizer" && *t == IqTier::Scholar));
    }

    #[test]
    fn default_citation_verifier_accepts_nonempty() {
        assert!(FixtureCitationVerifier.verify("anything"));
        assert!(!FixtureCitationVerifier.verify(""));
    }
}
