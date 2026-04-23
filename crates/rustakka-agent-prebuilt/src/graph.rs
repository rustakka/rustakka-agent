//! Graph layer — real `rustakka-langgraph` types, with a thin
//! topology **blueprint** for pre-compile inspection.
//!
//! Before this crate's v0, we kept a private "seam" here that mirrored
//! upstream. Now that `rustakka-agent-prebuilt` depends on
//! `rustakka-langgraph` directly, most of that seam is gone — we just
//! re-export upstream types and provide the two small glue pieces that
//! bridge our pure-data crates (`iq`, `persona`) to the engine:
//!
//! - a blanket [`iq::CallOptionsLike`](rustakka_agent_iq::ladder::CallOptionsLike)
//!   impl for [`CallOptions`] (`top_p` routed through `extra`);
//! - a blanket [`iq::ChatModel`](rustakka_agent_iq::ladder::ChatModel)
//!   impl for every upstream [`ProviderChatModel`];
//! - a [`Blueprint`] — a serializable topology description used for
//!   unit tests, mermaid rendering, and snapshot fixtures.
//!
//! Code that used to return `CompiledGraph` (our old fake) now returns
//! [`AgentGraph`], which *contains* the real
//! [`CompiledStateGraph`](rustakka_langgraph_core::graph::CompiledStateGraph)
//! plus a [`Blueprint`] so tests can still assert topology.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use rustakka_agent_iq::ladder::ChatModel as AgentChatModel;
pub use rustakka_agent_iq::ladder::ProviderModel;

// ---------------------------------------------------------------------------
// Upstream re-exports — the authoritative types live there.
// ---------------------------------------------------------------------------

pub use rustakka_langgraph_core::context::StoreAccessor;
pub use rustakka_langgraph_core::graph::{
    CompileConfig, CompiledStateGraph, StateGraph, END, START,
};
pub use rustakka_langgraph_core::node::{NodeKind, NodeOutput};
pub use rustakka_langgraph_core::state::{ChannelSpec as UpstreamChannelSpec, DynamicState};
pub use rustakka_langgraph_prebuilt::tool_node::{
    Tool as UpstreamTool, ToolFn, ToolNode, ToolNodeOptions,
};
pub use rustakka_langgraph_providers::prelude::{
    CallOptions, ChatModel as ProviderChatModel, ContentBlock, Message, Role, ToolDefinition,
};
pub use rustakka_langgraph_providers::traits::chat_model_stream_source;
pub use rustakka_langgraph_store::{store_accessor, BaseStore, InMemoryStore};

pub type GraphResult<T> = rustakka_langgraph_core::errors::GraphResult<T>;
pub type GraphError = rustakka_langgraph_core::errors::GraphError;

// ---------------------------------------------------------------------------
// `CallOptions` and `ChatModel` adapters live in `rustakka-agent-iq`
// behind its `langgraph` feature, so the pure-data iq crate can remain
// graph-free while this crate gets the blanket impls for free.
// ---------------------------------------------------------------------------

/// Lift an upstream `Arc<dyn ProviderChatModel>` into a persona-side
/// `Arc<dyn AgentChatModel>`. Cheap — just wraps the arc.
pub fn provider_as_agent_model(m: Arc<dyn ProviderChatModel>) -> Arc<dyn AgentChatModel> {
    Arc::new(ProviderModel(m))
}

// ---------------------------------------------------------------------------
// Agent-facing `Tool` — upstream's tool struct is runtime-only
// (carries a closure), so patterns and tests that want declarative
// topologies use this tag-richer descriptor and convert to the runtime
// `UpstreamTool` at compile time.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tool {
    pub name: String,
    pub description: String,
    /// Free-form tag (`"search"`, `"math"`, …) used by persona-aware
    /// middleware (allow-lists, bias, suppression).
    pub category: Option<String>,
}

impl Tool {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            category: None,
        }
    }
    pub fn with_category(mut self, c: impl Into<String>) -> Self {
        self.category = Some(c.into());
        self
    }

    /// Materialize to an upstream runtime tool, supplying the closure
    /// the node will actually run.
    pub fn into_runtime(self, func: ToolFn) -> UpstreamTool {
        UpstreamTool {
            name: self.name,
            description: self.description,
            func,
        }
    }
}

// ---------------------------------------------------------------------------
// ChannelKind — our patterns' catalogue-level shorthand. Translates
// to upstream's string-identified reducer via `ChannelKind::as_reducer`.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelKind {
    LastValue,
    AppendList,
    Messages,
}

impl ChannelKind {
    pub fn as_reducer(self) -> &'static str {
        match self {
            ChannelKind::LastValue => "last_value",
            ChannelKind::AppendList => "topic",
            ChannelKind::Messages => "add_messages",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelSpec {
    pub name: String,
    pub kind: ChannelKind,
}

impl ChannelSpec {
    pub fn last(name: &str) -> Self {
        Self { name: name.into(), kind: ChannelKind::LastValue }
    }
    pub fn appended(name: &str) -> Self {
        Self { name: name.into(), kind: ChannelKind::AppendList }
    }
    pub fn messages(name: &str) -> Self {
        Self { name: name.into(), kind: ChannelKind::Messages }
    }

    pub fn to_upstream(&self) -> UpstreamChannelSpec {
        UpstreamChannelSpec {
            name: self.name.clone(),
            reducer: self.kind.as_reducer().into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Blueprint — serializable topology description, used for pattern
// tests, mermaid rendering, and configuration roundtrips. The
// blueprint is what pattern builders return; callers then call
// `Blueprint::compile(...)` to produce a real `CompiledStateGraph`.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blueprint {
    pub name: String,
    pub nodes: Vec<String>,
    pub edges: Vec<(String, String)>,
    pub channels: BTreeMap<String, ChannelKind>,
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub recursion_limit: Option<u32>,
    /// Node names that should pause before running — maps to
    /// upstream's `CompileConfig::interrupt_before`.
    #[serde(default)]
    pub interrupt_before: Vec<String>,
    /// Node names that should pause after running — maps to
    /// upstream's `CompileConfig::interrupt_after`.
    #[serde(default)]
    pub interrupt_after: Vec<String>,
}

impl Blueprint {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), ..Self::default() }
    }
    pub fn add_node(&mut self, name: impl Into<String>) -> &mut Self {
        let n = name.into();
        if !self.nodes.iter().any(|x| x == &n) {
            self.nodes.push(n);
        }
        self
    }
    pub fn add_edge(&mut self, from: impl Into<String>, to: impl Into<String>) -> &mut Self {
        self.edges.push((from.into(), to.into()));
        self
    }
    pub fn has_node(&self, n: &str) -> bool {
        self.nodes.iter().any(|x| x == n)
    }
    pub fn has_edge(&self, from: &str, to: &str) -> bool {
        self.edges.iter().any(|(f, t)| f == from && t == to)
    }
    pub fn declare(&mut self, spec: &ChannelSpec) -> &mut Self {
        self.channels.insert(spec.name.clone(), spec.kind);
        self
    }

    /// Compile this blueprint into a real upstream `CompiledStateGraph`.
    ///
    /// Every node is wired with a `no-op` function that just forwards
    /// channel state unchanged; callers who want real node behavior
    /// build the graph themselves via [`StateGraph`]. The blueprint is
    /// primarily a *topology witness* for snapshot tests and docs.
    pub async fn compile(&self) -> GraphResult<CompiledStateGraph> {
        let mut g = StateGraph::<DynamicState>::new();
        for (name, kind) in &self.channels {
            g.add_channel(UpstreamChannelSpec {
                name: name.clone(),
                reducer: kind.as_reducer().into(),
            });
        }
        for n in &self.nodes {
            if n == "start" || n == "end" || n == START || n == END {
                continue;
            }
            g.add_node(n.clone(), noop_node())?;
        }
        for (from, to) in &self.edges {
            let f = translate_sentinel(from);
            let t = translate_sentinel(to);
            g.add_edge(f, t);
        }
        let cfg = CompileConfig {
            recursion_limit: self.recursion_limit,
            interrupt_before: self.interrupt_before.clone(),
            interrupt_after: self.interrupt_after.clone(),
            ..CompileConfig::default()
        };
        g.compile(cfg).await
    }
}

fn translate_sentinel(s: &str) -> &str {
    match s {
        "start" => START,
        "end" => END,
        other => other,
    }
}

/// Node that emits no updates — used by `Blueprint::compile` for
/// topology-only scaffolds.
pub fn noop_node() -> NodeKind {
    NodeKind::from_fn(|_input: BTreeMap<String, Value>| async move {
        Ok(NodeOutput::Update(BTreeMap::new()))
    })
}

// ---------------------------------------------------------------------------
// AgentGraph — what our `create_persona_*` / pattern `compile_runnable`
// fns return: a ready-to-run compiled graph, plus the blueprint that
// produced it (for assertion / mermaid / diagnostics).
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AgentGraph {
    pub blueprint: Blueprint,
    pub call_options: CallOptions,
    pub tools: Vec<Tool>,
    pub model: Option<Arc<dyn ProviderChatModel>>,
    pub compiled: Arc<CompiledStateGraph>,
    /// Optional long-term key/value store that downstream runners
    /// should attach to the graph via
    /// [`rustakka_langgraph_core::runner::invoke_with_store`].
    /// Populated by `omo_harness` and any other builder that wants
    /// cross-session continuity.
    pub store: Option<Arc<dyn BaseStore>>,
}

impl std::fmt::Debug for AgentGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentGraph")
            .field("blueprint", &self.blueprint)
            .field("call_options", &self.call_options)
            .field("tools", &self.tools)
            .field("model", &self.model.as_ref().map(|m| m.model_name().to_string()))
            .field("compiled.node_count", &self.compiled.topology().nodes.len())
            .finish()
    }
}

impl AgentGraph {
    /// Convenience: render the real upstream graph as mermaid.
    pub fn draw_mermaid(&self) -> String {
        self.compiled.draw_mermaid()
    }
    pub fn has_node(&self, n: &str) -> bool {
        self.blueprint.has_node(n)
    }
    pub fn has_edge(&self, from: &str, to: &str) -> bool {
        self.blueprint.has_edge(from, to)
    }

    /// Lift [`Self::store`] into a runtime `StoreAccessor` ready to
    /// hand to [`rustakka_langgraph_core::runner::invoke_with_store`].
    /// Returns `None` iff no store was attached to this agent.
    pub fn store_accessor(&self) -> Option<Arc<dyn StoreAccessor>> {
        self.store.as_ref().map(|s| store_accessor_from_dyn(s.clone()))
    }
}

// `store_accessor::<S: BaseStore>` is generic, but we carry an
// `Arc<dyn BaseStore>`; wrap via a tiny newtype so the generic bound
// is satisfied.
fn store_accessor_from_dyn(inner: Arc<dyn BaseStore>) -> Arc<dyn StoreAccessor> {
    store_accessor(Arc::new(DynStoreShim(inner)))
}

struct DynStoreShim(Arc<dyn BaseStore>);

#[async_trait::async_trait]
impl BaseStore for DynStoreShim {
    async fn get(
        &self,
        namespace: &rustakka_langgraph_store::base::Namespace,
        key: &str,
    ) -> GraphResult<Option<rustakka_langgraph_store::base::Item>> {
        self.0.get(namespace, key).await
    }
    async fn put(
        &self,
        namespace: &rustakka_langgraph_store::base::Namespace,
        key: &str,
        value: Value,
        opts: rustakka_langgraph_store::base::PutOptions,
    ) -> GraphResult<()> {
        self.0.put(namespace, key, value, opts).await
    }
    async fn delete(
        &self,
        namespace: &rustakka_langgraph_store::base::Namespace,
        key: &str,
    ) -> GraphResult<()> {
        self.0.delete(namespace, key).await
    }
    async fn search(
        &self,
        namespace_prefix: &rustakka_langgraph_store::base::Namespace,
        query: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> GraphResult<Vec<rustakka_langgraph_store::base::SearchHit>> {
        self.0.search(namespace_prefix, query, limit, offset).await
    }
    async fn list_namespaces(
        &self,
        filter: rustakka_langgraph_store::base::ListNamespacesFilter,
    ) -> GraphResult<Vec<rustakka_langgraph_store::base::Namespace>> {
        self.0.list_namespaces(filter).await
    }
}

// ---------------------------------------------------------------------------
// Mock helpers — use the real upstream mock provider under the hood so
// snapshot tests get actual runnable graphs, not a fake.
// ---------------------------------------------------------------------------

pub mod mock {
    use super::*;
    use rustakka_langgraph_providers::prelude::MockChatModel;

    /// Build a deterministic mock provider that returns a single
    /// canned assistant message. Name defaults to `"mock"`; callers
    /// who care about identity pass a different one.
    pub fn echo_provider(name: &'static str) -> Arc<dyn ProviderChatModel> {
        Arc::new(MockChatModel::new(vec![Message::ai("")]).with_name(name))
    }

    /// Backwards-compatible alias — tests that want "just a model"
    /// typed against our agent-side `ChatModel` trait use this.
    pub fn echo_agent_model(name: &'static str) -> Arc<dyn AgentChatModel> {
        provider_as_agent_model(echo_provider(name))
    }

    /// Legacy named constructor kept so existing example code still
    /// reads naturally.
    #[derive(Debug)]
    pub struct EchoModel(pub &'static str);

    impl AgentChatModel for EchoModel {
        fn model_name(&self) -> &str {
            self.0
        }
        fn is_mock(&self) -> bool {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn blueprint_roundtrips_to_real_compiled_graph() {
        let mut bp = Blueprint::new("t");
        bp.add_node("a").add_node("b");
        bp.add_edge("start", "a").add_edge("a", "b").add_edge("b", "end");
        bp.declare(&ChannelSpec::messages("messages"));
        let compiled = bp.compile().await.unwrap();
        assert!(compiled.topology().nodes.contains_key("a"));
        assert!(compiled.topology().nodes.contains_key("b"));
    }

    #[test]
    fn call_options_adapter_routes_top_p_into_extra() {
        let mut o = CallOptions::default();
        let c = rustakka_agent_iq::ladder::IqCarryings {
            temperature: Some(0.5),
            top_p: Some(0.5),
            max_tokens: Some(512),
            ..Default::default()
        };
        c.apply_to(&mut o);
        assert_eq!(o.temperature, Some(0.5));
        assert_eq!(o.max_tokens, Some(512));
        let p = o
            .extra
            .get("top_p")
            .and_then(|v| v.as_f64())
            .expect("top_p stored in extra");
        assert!((p - 0.5).abs() < 1e-5);
    }
}
