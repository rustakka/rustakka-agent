//! Phase 4 — persona-aware supervisor and swarm.
//!
//! A [`PersonaAgent`] couples a [`Persona`] with a compiled ReAct
//! subgraph. Supervisor and swarm factories accept a collection of
//! [`PersonaAgent`]s and wire them together using upstream's
//! [`rustakka_langgraph_prebuilt::supervisor::create_supervisor`] and
//! [`rustakka_langgraph_prebuilt::swarm::create_swarm`]. Routing knobs
//! (round-robin, by-category, persona-aware) are produced as closures
//! the upstream `create_supervisor` consumes directly.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::Value;

use rustakka_agent_persona::Persona;
use rustakka_langgraph_core::graph::END;
use rustakka_langgraph_core::node::{NodeKind, NodeOutput};
use rustakka_langgraph_prebuilt::supervisor::{
    create_supervisor, Agent as UpstreamAgent, SupervisorRouter as UpstreamRouter,
};
use rustakka_langgraph_prebuilt::swarm::create_swarm;
use rustakka_langgraph_providers::prelude::ChatModel as ProviderChatModel;

use crate::graph::{AgentGraph, Blueprint, ChannelKind, GraphResult, Tool};
use crate::react::{create_persona_react_agent, AgentOptions, PersonaReactAgent, ReactAgentOptions};

/// A single named agent in a supervisor / swarm: a persona plus its
/// compiled ReAct subgraph plus declared categories.
#[derive(Clone)]
pub struct PersonaAgent {
    pub name: String,
    pub categories: Vec<String>,
    pub inner: PersonaReactAgent,
}

impl std::fmt::Debug for PersonaAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PersonaAgent")
            .field("name", &self.name)
            .field("categories", &self.categories)
            .finish()
    }
}

impl PersonaAgent {
    /// Compile a [`PersonaAgent`] from a persona + tools.
    pub async fn new(
        name: impl Into<String>,
        model: Arc<dyn ProviderChatModel>,
        persona: Persona,
        tools: Vec<Tool>,
        categories: Vec<String>,
    ) -> GraphResult<Self> {
        let inner = create_persona_react_agent(
            model,
            tools,
            AgentOptions {
                persona: Some(persona),
                react: ReactAgentOptions::default(),
            },
        )
        .await?;
        Ok(Self {
            name: name.into(),
            categories,
            inner,
        })
    }
}

/// Routing policy used by the supervisor.
#[derive(Clone, Debug, Default)]
pub enum SupervisorRouter {
    /// Round-robin across agents regardless of content.
    #[default]
    RoundRobin,
    /// Inspect the orchestrator's declared intent (a channel written
    /// by the orchestrator node) and route to the discipline whose
    /// categories list matches.
    ByCategory,
    /// Prefer the agent whose persona lists the most overlapping
    /// `knowledge_domains` / `values` with the request hint. The
    /// request hint is a free-form string stored in a graph channel.
    PersonaAware,
}

/// Persona-aware router: given a request hint and a set of agents,
/// returns the best-fit agent's index using a simple overlap score
/// over knowledge domains, values, and categories.
pub fn persona_based_router<'a>(
    agents: &'a [PersonaAgent],
    hint: &str,
) -> Option<&'a PersonaAgent> {
    if agents.is_empty() {
        return None;
    }
    let hint_lc = hint.to_ascii_lowercase();
    let score = |a: &PersonaAgent| -> usize {
        let mut score = 0usize;
        if let Some(p) = &a.inner.persona {
            for d in &p.knowledge_domains {
                if hint_lc.contains(&d.to_ascii_lowercase()) {
                    score += 3;
                }
            }
            for v in &p.values {
                if hint_lc.contains(&v.to_ascii_lowercase()) {
                    score += 1;
                }
            }
        }
        for c in &a.categories {
            if hint_lc.contains(&c.to_ascii_lowercase()) {
                score += 2;
            }
        }
        score
    };

    agents
        .iter()
        .max_by_key(|a| (score(a), std::cmp::Reverse(a.name.clone())))
}

/// Build a [`UpstreamRouter`] matching `policy`. Reads the orchestrator
/// hint (if any) from the `"hint"` channel and a tick counter from
/// `"tick"` for round-robin.
fn build_router(policy: &SupervisorRouter, agents: &[PersonaAgent]) -> UpstreamRouter {
    let names: Vec<String> = agents.iter().map(|a| a.name.clone()).collect();
    let cats: Vec<(String, Vec<String>)> = agents
        .iter()
        .map(|a| (a.name.clone(), a.categories.clone()))
        .collect();
    let persona_hints: Vec<(String, Vec<String>)> = agents
        .iter()
        .map(|a| {
            let mut h = a.categories.clone();
            if let Some(p) = &a.inner.persona {
                h.extend(p.knowledge_domains.iter().cloned());
                h.extend(p.values.iter().cloned());
            }
            (a.name.clone(), h)
        })
        .collect();

    match policy {
        SupervisorRouter::RoundRobin => {
            let names = names.clone();
            let tick = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            Arc::new(move |_vals: &BTreeMap<String, Value>| {
                if names.is_empty() {
                    return vec![END.into()];
                }
                let i = tick.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                vec![names[i % names.len()].clone()]
            })
        }
        SupervisorRouter::ByCategory => {
            let cats = cats.clone();
            Arc::new(move |vals: &BTreeMap<String, Value>| {
                let hint = vals
                    .get("hint")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                for (name, c) in &cats {
                    if c.iter().any(|cc| hint.contains(&cc.to_ascii_lowercase())) {
                        return vec![name.clone()];
                    }
                }
                vec![END.into()]
            })
        }
        SupervisorRouter::PersonaAware => {
            let ph = persona_hints.clone();
            Arc::new(move |vals: &BTreeMap<String, Value>| {
                let hint = vals
                    .get("hint")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let scored = ph
                    .iter()
                    .map(|(n, keys)| {
                        let s = keys
                            .iter()
                            .filter(|k| hint.contains(&k.to_ascii_lowercase()))
                            .count();
                        (s, n.clone())
                    })
                    .max_by_key(|(s, n)| (*s, std::cmp::Reverse(n.clone())));
                match scored {
                    Some((s, n)) if s > 0 => vec![n],
                    _ => vec![END.into()],
                }
            })
        }
    }
}

/// Wrap an [`AgentGraph`] as an upstream [`UpstreamAgent`] whose node
/// is the agent's compiled subgraph invoked as a subgraph.
fn agent_to_upstream(a: &PersonaAgent) -> UpstreamAgent {
    let invoker = a.inner.graph.compiled.clone().as_subgraph_invoker();
    UpstreamAgent::new(a.name.clone(), NodeKind::Subgraph(invoker))
}

/// Compile a persona-aware supervisor over a collection of agents.
///
/// The `supervisor` persona provides the top-level system prompt /
/// call options. Its own ReAct subgraph is used as the supervisor
/// node, so the supervisor can reason about tool selection the same
/// way any other ReAct agent would — the [`SupervisorRouter`] merely
/// reads its final state to pick the next hop.
pub async fn create_persona_supervisor(
    supervisor: PersonaAgent,
    router: SupervisorRouter,
    agents: Vec<PersonaAgent>,
) -> GraphResult<AgentGraph> {
    let upstream_agents: Vec<UpstreamAgent> = agents.iter().map(agent_to_upstream).collect();
    let router_fn = build_router(&router, &agents);
    let supervisor_node =
        NodeKind::Subgraph(supervisor.inner.graph.compiled.clone().as_subgraph_invoker());
    let compiled = create_supervisor(supervisor_node, router_fn, upstream_agents).await?;

    // Blueprint reflects the hub-and-spoke topology for tests / diagrams.
    let mut blueprint = Blueprint::new("persona_supervisor");
    blueprint.add_node("supervisor");
    blueprint.add_edge("start", "supervisor");
    for a in &agents {
        blueprint.add_node(&a.name);
        blueprint.add_edge("supervisor", &a.name);
        blueprint.add_edge(&a.name, "supervisor");
    }
    blueprint.add_edge("supervisor", "end");
    blueprint
        .channels
        .insert("supervisor.hint".into(), ChannelKind::LastValue);
    blueprint.channels.insert(
        format!("supervisor.router.{router:?}"),
        ChannelKind::LastValue,
    );
    blueprint.system_prompt = supervisor
        .inner
        .persona
        .as_ref()
        .map(|p| p.to_system_prompt())
        .filter(|s| !s.is_empty());

    Ok(AgentGraph {
        blueprint,
        call_options: supervisor.inner.graph.call_options.clone(),
        tools: Vec::new(),
        model: supervisor.inner.graph.model.clone(),
        compiled: Arc::new(compiled),
        store: None,
    })
}

/// Compile a swarm: a fully-connected set of persona agents that can
/// hand off to each other without a supervisor. Uses upstream's
/// [`create_swarm`] so handoffs via the `next` channel are the
/// authoritative mechanism.
pub async fn create_persona_swarm(
    agents: Vec<PersonaAgent>,
) -> GraphResult<AgentGraph> {
    let default = agents
        .first()
        .map(|a| a.name.clone())
        .unwrap_or_else(|| "agent".into());
    let upstream_agents: Vec<UpstreamAgent> = agents.iter().map(agent_to_upstream).collect();
    let compiled = create_swarm(upstream_agents, default).await?;

    let mut blueprint = Blueprint::new("persona_swarm");
    for a in &agents {
        blueprint.add_node(&a.name);
    }
    for a in &agents {
        for b in &agents {
            if a.name != b.name {
                blueprint.add_edge(&a.name, &b.name);
            }
        }
        blueprint.add_edge(&a.name, "end");
    }

    Ok(AgentGraph {
        blueprint,
        call_options: Default::default(),
        tools: Vec::new(),
        model: None,
        compiled: Arc::new(compiled),
        store: None,
    })
}

// Helper: a no-op node factory for tests that only need a wired graph.
#[allow(dead_code)]
fn noop_node() -> NodeKind {
    NodeKind::from_fn(|_| async move { Ok(NodeOutput::Update(BTreeMap::new())) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::mock::echo_provider;

    fn model() -> Arc<dyn ProviderChatModel> {
        echo_provider("echo")
    }

    async fn agent(name: &str, domains: Vec<String>, categories: Vec<String>) -> PersonaAgent {
        let persona = Persona::builder()
            .name(name)
            .knowledge_domains(domains)
            .build();
        PersonaAgent::new(name, model(), persona, vec![], categories)
            .await
            .unwrap()
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn persona_based_router_prefers_domain_overlap() {
        let math = agent(
            "math",
            vec!["mathematics".into()],
            vec!["reasoning".into()],
        )
        .await;
        let chef = agent("chef", vec!["cooking".into()], vec!["recipes".into()]).await;
        let agents = [math, chef];
        let pick = persona_based_router(&agents, "help me with mathematics");
        assert_eq!(pick.unwrap().name, "math");
        let pick = persona_based_router(&agents, "what recipes do you recommend?");
        assert_eq!(pick.unwrap().name, "chef");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn supervisor_wires_every_discipline() {
        let supervisor = agent("boss", vec![], vec!["orchestration".into()]).await;
        let a = agent("alice", vec![], vec!["a".into()]).await;
        let b = agent("bob", vec![], vec!["b".into()]).await;
        let g = create_persona_supervisor(
            supervisor,
            SupervisorRouter::ByCategory,
            vec![a, b],
        )
        .await
        .unwrap();
        assert!(g.blueprint.has_node("supervisor"));
        assert!(g.blueprint.has_edge("supervisor", "alice"));
        assert!(g.blueprint.has_edge("alice", "supervisor"));
        assert!(g.blueprint.has_edge("bob", "supervisor"));
        // Upstream compiled graph actually wired the agents too.
        assert!(g.compiled.topology().nodes.contains_key("alice"));
        assert!(g.compiled.topology().nodes.contains_key("bob"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn swarm_is_fully_connected() {
        let a = agent("a", vec![], vec![]).await;
        let b = agent("b", vec![], vec![]).await;
        let c = agent("c", vec![], vec![]).await;
        let g = create_persona_swarm(vec![a, b, c]).await.unwrap();
        for (from, to) in [("a", "b"), ("b", "c"), ("c", "a"), ("a", "c")] {
            assert!(
                g.blueprint.has_edge(from, to),
                "blueprint missing edge {from}→{to}"
            );
        }
        assert!(g.compiled.topology().nodes.contains_key("a"));
    }
}
