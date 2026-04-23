//! # rustakka-agent-prebuilt
//!
//! Persona-aware adapters and opinionated prebuilt agent graphs.
//!
//! ## Layering
//!
//! This crate is the single bridge between the `rustakka-agent`
//! family (pure data + logic) and the `rustakka-langgraph` graph
//! engine. The seam is kept narrow so that `iq` / `eq` / `persona`
//! stay reusable in non-graph contexts (validators, fixture
//! generators, CLI tools).
//!
//! ### Upstream coupling strategy
//!
//! This crate now depends **directly** on the
//! [`rustakka-langgraph`](https://github.com/rustakka/rustakka-langgraph)
//! workspace (`rustakka-langgraph-core`,
//! `rustakka-langgraph-providers`, `rustakka-langgraph-prebuilt`,
//! `rustakka-langgraph-store`). Every persona-aware builder
//! delegates to the upstream prebuilts (`create_react_agent`,
//! `create_supervisor`, `create_swarm`) and returns a real
//! [`rustakka_langgraph_core::graph::CompiledStateGraph`].
//!
//! Two small shims keep the layering clean:
//!
//! - [`graph::Blueprint`] — a serializable topological description
//!   (nodes, edges, channels, system prompt, recursion limit,
//!   interrupt points). Patterns and persona builders produce one so
//!   tests can assert structure without running the graph.
//! - [`graph::AgentGraph`] — bundles a `Blueprint` with the real
//!   compiled graph, the `CallOptions`, the tool list, and the
//!   `ChatModel`.
//!
//! The `CallOptionsLike` / `ChatModel` traits live in
//! `rustakka-agent-iq` (so the characteristics crate stays usable in
//! non-graph contexts); blanket impls against the upstream provider
//! types are gated behind the `langgraph` feature of
//! `rustakka-agent-iq`, which this crate enables.
//!
//! ## Entry points
//!
//! - [`create_persona_react_agent`] (Phase 3)
//! - [`create_persona_supervisor`] / [`create_persona_swarm`] (Phase 4)
//! - [`aiq_research::create_aiq_research_agent`] (Phase 5a, feature `aiq-research`)
//! - [`omo_harness::create_omo_harness`] (Phase 5b, feature `omo-harness`)
//! - [`patterns::*`] (Phase 5c, feature `patterns`)

pub mod graph;

mod react;
pub use react::{
    create_persona_react_agent, AgentOptions, PersonaReactAgent, ReactAgentLike,
    ReactAgentOptions,
};

mod supervisor;
pub use supervisor::{
    create_persona_supervisor, create_persona_swarm, persona_based_router, PersonaAgent,
    SupervisorRouter,
};

mod reflection;
pub use reflection::{inject_reflection, tool_bias_from_iq, ToolBias};

pub mod patterns;

#[cfg(feature = "aiq-research")]
pub mod aiq_research;

#[cfg(feature = "omo-harness")]
pub mod omo_harness;
