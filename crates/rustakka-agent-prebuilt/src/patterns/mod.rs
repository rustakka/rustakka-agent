//! Common agentic patterns (Phase 5c).
//!
//! Every pattern in this module implements the shared [`Pattern`]
//! trait. A pattern's `compile` returns a [`Blueprint`] — a
//! declarative topology description — that callers can:
//!
//! - assert on structurally (tests, docs, mermaid renders);
//! - lift into a real [`CompiledStateGraph`](crate::graph::CompiledStateGraph)
//!   via [`Blueprint::compile`] or the convenience
//!   [`Pattern::compile_runnable`] extension method.
//!
//! See `docs/patterns.md` for the catalog, topologies, and per-pattern
//! channel namespaces.
//!
//! ## Role → tier mapping
//!
//! Every pattern factory accepts a [`RoleTierMap`]. When a caller
//! binds a pattern to an [`IqLadder`](rustakka_agent_iq::IqLadder), the
//! pattern uses `RoleTierMap` to pick the rung for each internal role
//! (planner, executor, critic, evaluator, aggregator, …).

use std::collections::BTreeMap;
use std::sync::Arc;

use rustakka_agent_iq::IqTier;

use crate::graph::{Blueprint, ChannelSpec, CompiledStateGraph, GraphResult};

pub mod adaptive_rag;
pub mod codex_loop;
pub mod crag;
pub mod debate;
pub mod eval_opt;
pub mod guardrails;
pub mod hitl_gate;
pub mod memory_agent;
pub mod plan_execute;
pub mod rag;
pub mod reflexion;
pub mod router;
pub mod self_consistency;
pub mod self_rag;
pub mod tot;

/// Core capability of every pattern in the catalog.
pub trait Pattern {
    /// Crate-stable name, e.g. `"plan_execute"`.
    fn name(&self) -> &'static str;

    /// Channel specs this pattern writes or reads.
    fn channels(&self) -> Vec<ChannelSpec>;

    /// Compile to a topology blueprint.
    fn compile(&self) -> GraphResult<Blueprint>;
}

/// Extension that turns any [`Pattern`] into a real upstream compiled
/// graph. Kept as a separate trait so the synchronous
/// [`Pattern::compile`] stays easy to call in tests and docs.
pub trait PatternRunnable: Pattern {
    fn compile_runnable(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = GraphResult<Arc<CompiledStateGraph>>> + Send + '_>>;
}

impl<P: Pattern + Sync + ?Sized> PatternRunnable for P {
    fn compile_runnable(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = GraphResult<Arc<CompiledStateGraph>>> + Send + '_>> {
        Box::pin(async move {
            let bp = self.compile()?;
            let compiled = bp.compile().await?;
            Ok(Arc::new(compiled))
        })
    }
}

/// Map from pattern-specific role names to IQ tiers. Overridable per
/// pattern-builder.
#[derive(Clone, Debug, Default)]
pub struct RoleTierMap(pub BTreeMap<String, IqTier>);

impl RoleTierMap {
    pub fn with(mut self, role: impl Into<String>, tier: IqTier) -> Self {
        self.0.insert(role.into(), tier);
        self
    }
    pub fn get(&self, role: &str) -> Option<IqTier> {
        self.0.get(role).copied()
    }
}

/// Helpers shared between every individual pattern module.
pub(crate) fn fresh_blueprint(name: &'static str) -> Blueprint {
    let mut g = Blueprint::new(name);
    g.add_node("start");
    g.add_node("end");
    g
}

pub(crate) fn register_channels(g: &mut Blueprint, specs: &[ChannelSpec]) {
    for s in specs {
        g.channels.insert(s.name.clone(), s.kind);
    }
}
