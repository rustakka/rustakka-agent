//! Oh-My-OpenAgent-style harness graph (Phase 5b).
//!
//! Modeled after [Oh My OpenAgent](https://github.com/code-yeongyu/oh-my-openagent):
//! an orchestrator (`sisyphus`) routes tasks by category to a set of
//! discipline personas (planner, deep worker, oracle, librarian,
//! explorer, visio, quick).
//!
//! A `BoulderStore` channel provides session continuity across
//! checkpoints; a `HashlineGate` middleware rejects edits whose
//! anchor-hash has drifted.
//!
//! The cross-session store is the authoritative upstream
//! [`rustakka_langgraph_store::BaseStore`] — we do not redefine it.

use std::sync::Arc;

use rustakka_agent_iq::{IqLadder, IqTier};
use rustakka_agent_persona::Persona;

pub use crate::graph::{BaseStore, InMemoryStore};

use crate::graph::{AgentGraph, ChannelKind, GraphResult, Tool};
use crate::supervisor::{create_persona_supervisor, PersonaAgent, SupervisorRouter};

pub mod channels {
    pub const BOULDER: &str = "omo.boulder";
    pub const HASHLINE: &str = "omo.hashline";
}

/// Edit-safety mode for the hashline gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HashlineMode {
    Off,
    Warn,
    #[default]
    Enforce,
}

/// Harness options. See `docs/omo-harness.md` for usage recipes.
#[derive(Clone)]
pub struct OmoHarnessOptions {
    pub ladder: IqLadder,
    pub orchestrator: PersonaAgent,
    pub disciplines: Vec<PersonaAgent>,
    /// Optional upstream store used for cross-session continuity.
    /// Typical prod deployments wire a Postgres-backed
    /// [`rustakka_langgraph_store::BaseStore`] here; unit tests use
    /// [`InMemoryStore`].
    pub boulder_store: Option<Arc<dyn BaseStore>>,
    pub hashline: HashlineMode,
    pub default_set: bool,
}

impl std::fmt::Debug for OmoHarnessOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OmoHarnessOptions")
            .field("orchestrator", &self.orchestrator.name)
            .field(
                "disciplines",
                &self.disciplines.iter().map(|a| &a.name).collect::<Vec<_>>(),
            )
            .field("hashline", &self.hashline)
            .field("default_set", &self.default_set)
            .finish()
    }
}

/// Canonical discipline set.
pub fn default_disciplines_spec() -> Vec<(&'static str, IqTier, &'static str)> {
    vec![
        ("sisyphus", IqTier::Strategist, "orchestration"),
        ("prometheus", IqTier::Strategist, "planning"),
        ("hephaestus", IqTier::Scholar, "deep"),
        ("oracle", IqTier::Strategist, "ultrabrain"),
        ("librarian", IqTier::Analyst, "documentation"),
        ("explore", IqTier::Operator, "search"),
        ("visio", IqTier::Analyst, "visual-engineering"),
        ("quick", IqTier::Reflex, "quick"),
    ]
}

/// Compile the OMO harness. Under the hood this delegates to
/// [`create_persona_supervisor`] and records session-continuity +
/// hashline-gate state as blueprint channels so downstream tooling
/// can inspect the configuration.
pub async fn create_omo_harness(opts: OmoHarnessOptions) -> GraphResult<AgentGraph> {
    let mut graph = create_persona_supervisor(
        opts.orchestrator.clone(),
        SupervisorRouter::PersonaAware,
        opts.disciplines.clone(),
    )
    .await?;

    if opts.boulder_store.is_some() {
        graph
            .blueprint
            .channels
            .insert(channels::BOULDER.into(), ChannelKind::LastValue);
    }
    graph.blueprint.channels.insert(
        format!("omo.hashline.mode.{:?}", opts.hashline),
        ChannelKind::LastValue,
    );

    // Hand the store through to the compiled graph so runners can
    // call `AgentGraph::store_accessor()` and pass it to
    // `invoke_with_store`.
    graph.store = opts.boulder_store;

    Ok(graph)
}

/// Build the canonical discipline agents against an `IqLadder`.
///
/// Returns [`PersonaAgent`]s that are already compiled. Callers can
/// extend / filter this list before passing it to
/// [`create_omo_harness`].
pub async fn default_disciplines(
    ladder: &IqLadder,
    model: Arc<dyn rustakka_langgraph_providers::prelude::ChatModel>,
) -> GraphResult<Vec<PersonaAgent>> {
    let _ = ladder;
    let mut out = Vec::new();
    for (name, tier, category) in default_disciplines_spec() {
        let persona = Persona::builder()
            .name(name)
            .role(format!("{name} discipline"))
            .iq(rustakka_agent_iq::IqProfile::builder()
                .pin_tier(tier)
                .build())
            .knowledge_domains([category])
            .build();
        out.push(
            PersonaAgent::new(
                name,
                model.clone(),
                persona,
                vec![Tool::new(
                    format!("{name}_tool"),
                    format!("primary tool for {name}"),
                )],
                vec![category.to_string()],
            )
            .await?,
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::mock::echo_provider;
    use rustakka_agent_iq::IqLadder;

    async fn orchestrator() -> PersonaAgent {
        let persona = Persona::builder()
            .name("sisyphus")
            .knowledge_domains(["orchestration"])
            .build();
        PersonaAgent::new(
            "sisyphus",
            echo_provider("mock"),
            persona,
            vec![],
            vec!["orchestration".into()],
        )
        .await
        .unwrap()
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn defaults_produce_eight_disciplines() {
        let ladder = IqLadder::default();
        let agents = default_disciplines(&ladder, echo_provider("mock"))
            .await
            .unwrap();
        assert_eq!(agents.len(), 8);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn harness_wires_orchestrator_and_discipline_edges() {
        let ladder = IqLadder::default();
        let orch = orchestrator().await;
        let disciplines = default_disciplines(&ladder, echo_provider("mock"))
            .await
            .unwrap();
        let g = create_omo_harness(OmoHarnessOptions {
            ladder,
            orchestrator: orch,
            disciplines,
            boulder_store: Some(Arc::new(InMemoryStore::new())),
            hashline: HashlineMode::Enforce,
            default_set: true,
        })
        .await
        .unwrap();
        assert!(g.blueprint.has_edge("supervisor", "hephaestus"));
        assert!(g
            .blueprint
            .channels
            .keys()
            .any(|k| k.contains("omo.hashline.mode.Enforce")));
        assert!(
            g.store.is_some(),
            "harness must forward the boulder store onto the returned AgentGraph"
        );
        assert!(
            g.store_accessor().is_some(),
            "store_accessor() must be available for runners"
        );
    }
}
