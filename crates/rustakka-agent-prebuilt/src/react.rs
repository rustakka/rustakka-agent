//! Persona-aware ReAct agent (Phase 3).
//!
//! [`create_persona_react_agent`] is a thin, additive wrapper around
//! the upstream [`rustakka_langgraph_prebuilt::react_agent::create_react_agent`].
//! It accepts an `Option<Persona>`:
//!
//! - **Persona = None** → behaviorally identical to the upstream
//!   prebuilt (parity requirement from `docs/plan.md` § 3).
//! - **Persona = Some(_)** → the persona's `to_system_prompt` is
//!   injected (merged with any user-supplied `system_prompt`), its
//!   IQ knobs are folded into the provider's `CallOptions`, and its
//!   `recommended_recursion_limit` is applied to the compile config
//!   iff the caller hasn't pinned one. Optionally, a `reflect` node
//!   is spliced in when EQ requests it.
//!
//! The returned [`AgentGraph`] bundles the real upstream
//! [`CompiledStateGraph`] together with a [`Blueprint`] that callers
//! can introspect (mermaid rendering, snapshot tests, structural
//! assertions).

use std::sync::Arc;

use serde_json::{json, Value};

use rustakka_agent_persona::Persona;
use rustakka_langgraph_prebuilt::react_agent::{create_react_agent, ReactAgentOptions as UpstreamReactOptions};
use rustakka_langgraph_providers::prelude::{ChatModel as ProviderChatModel, Message};

use crate::graph::{
    AgentGraph, Blueprint, CallOptions, ChannelSpec, GraphResult, Tool, ToolFn, UpstreamTool,
};
use crate::reflection::{inject_reflection, tool_bias_from_iq};

/// Upstream-shaped ReAct options (persona-side subset).
#[derive(Clone, Debug, Default)]
pub struct ReactAgentOptions {
    pub system_prompt: Option<String>,
    pub recursion_limit: Option<u32>,
    pub call_options: CallOptions,
}

/// Persona + underlying ReAct options.
#[derive(Clone, Debug, Default)]
pub struct AgentOptions {
    pub persona: Option<Persona>,
    pub react: ReactAgentOptions,
}

/// Compiled persona-aware ReAct agent — an [`AgentGraph`] plus the
/// persona that produced it.
#[derive(Clone)]
pub struct PersonaReactAgent {
    pub persona: Option<Persona>,
    pub graph: AgentGraph,
}

impl std::fmt::Debug for PersonaReactAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PersonaReactAgent")
            .field("persona", &self.persona.as_ref().and_then(|p| p.identity.name.as_deref()))
            .field("graph", &self.graph)
            .finish()
    }
}

/// Persona-aware tool factory — callers can provide the actual
/// executable [`ToolFn`] alongside the tool metadata. For declarative
/// tests a [`PlaceholderToolFn`] echoes the call arguments as the
/// tool result.
pub trait ToolImplProvider: Send + Sync {
    fn tool_fn(&self, name: &str) -> Option<ToolFn>;
}

/// Default tool-impl provider that returns an echo-style closure for
/// every tool — sufficient for unit tests and persona snapshotting
/// where the agent is never actually invoked against a live LLM.
#[derive(Debug, Clone, Copy, Default)]
pub struct EchoToolImpls;

impl ToolImplProvider for EchoToolImpls {
    fn tool_fn(&self, _name: &str) -> Option<ToolFn> {
        Some(Arc::new(|args: Value| {
            Box::pin(async move {
                Ok::<Value, rustakka_langgraph_core::errors::GraphError>(json!({
                    "echo": args,
                }))
            })
        }))
    }
}

/// Compile a persona-aware ReAct agent on top of real
/// `rustakka-langgraph`.
///
/// The upstream `create_react_agent` builds the canonical
/// `agent ↔ tools` loop; on top of that we (a) fold persona knobs into
/// `CallOptions`, (b) merge the persona system prompt, (c) nudge the
/// recursion limit from `IqProfile::recommended_recursion_limit`, and
/// (d) record topology adjustments (reflect node, tool bias flags) on
/// the returned [`Blueprint`].
pub async fn create_persona_react_agent(
    model: Arc<dyn ProviderChatModel>,
    tools: Vec<Tool>,
    opts: AgentOptions,
) -> GraphResult<PersonaReactAgent> {
    let AgentOptions { persona, mut react } = opts;

    // ----- persona → prompt + call_options + recursion_limit -----
    if let Some(p) = &persona {
        let persona_prompt = p.to_system_prompt();
        react.system_prompt = Some(match react.system_prompt.take() {
            Some(user) if !persona_prompt.is_empty() => {
                format!("{persona_prompt}\n\n[User overrides]\n{user}")
            }
            Some(user) => user,
            None if !persona_prompt.is_empty() => persona_prompt,
            None => String::new(),
        });
        p.apply_to_call_options(&mut react.call_options);
        if react.recursion_limit.is_none() {
            if let Some(r) = p.iq.recommended_recursion_limit() {
                react.recursion_limit = Some(r);
            }
        }
    }
    let system_prompt = react
        .system_prompt
        .clone()
        .filter(|s| !s.is_empty());

    // ----- blueprint (canonical ReAct topology) -----
    let mut blueprint = Blueprint::new("persona_react");
    blueprint.add_node("agent");
    blueprint.add_node("tools");
    blueprint.add_edge("start", "agent");
    blueprint.add_edge("agent", "tools");
    blueprint.add_edge("tools", "agent");
    blueprint.add_edge("agent", "end");
    blueprint.declare(&ChannelSpec::messages("messages"));
    blueprint.system_prompt = system_prompt.clone();
    blueprint.recursion_limit = react.recursion_limit;

    if let Some(p) = &persona {
        inject_reflection(&mut blueprint, &p.eq);
        tool_bias_from_iq(&p.iq).apply(&mut blueprint);
    }

    // ----- real upstream compile -----
    let runtime_tools = materialize_tools(&tools, &EchoToolImpls);
    let model_fn = model_fn_for(model.clone(), react.call_options.clone());
    let compiled = create_react_agent(
        model_fn,
        runtime_tools,
        UpstreamReactOptions {
            system_prompt: system_prompt.clone(),
            recursion_limit: react.recursion_limit,
        },
    )
    .await?;

    Ok(PersonaReactAgent {
        persona,
        graph: AgentGraph {
            blueprint,
            call_options: react.call_options,
            tools,
            model: Some(model),
            compiled: Arc::new(compiled),
            store: None,
        },
    })
}

/// Turn our declarative [`Tool`]s into the upstream runtime
/// [`UpstreamTool`]s required by `create_react_agent`. The
/// [`ToolImplProvider`] supplies the closure each tool will run.
pub fn materialize_tools(tools: &[Tool], provider: &dyn ToolImplProvider) -> Vec<UpstreamTool> {
    tools
        .iter()
        .map(|t| {
            let func = provider.tool_fn(&t.name).unwrap_or_else(|| {
                Arc::new(|_v: Value| {
                    Box::pin(async move {
                        Err::<Value, _>(rustakka_langgraph_core::errors::GraphError::other(
                            "no tool implementation registered",
                        ))
                    })
                })
            });
            t.clone().into_runtime(func)
        })
        .collect()
}

/// Adapter that turns an `Arc<dyn ProviderChatModel>` + `CallOptions`
/// into the `ModelFn` shape `create_react_agent` expects.
pub fn model_fn_for(
    model: Arc<dyn ProviderChatModel>,
    call_options: CallOptions,
) -> rustakka_langgraph_prebuilt::react_agent::ModelFn {
    Arc::new(move |msgs: Vec<Value>, sys: Option<String>| {
        let model = model.clone();
        let call_options = call_options.clone();
        Box::pin(async move {
            let mut messages: Vec<Message> = Vec::new();
            if let Some(s) = sys {
                if !s.is_empty() {
                    messages.push(Message::system(s));
                }
            }
            for v in msgs {
                if let Ok(m) = serde_json::from_value::<Message>(v.clone()) {
                    messages.push(m);
                } else if let Some(text) = v.get("content").and_then(|c| c.as_str()) {
                    messages.push(Message::human(text.to_string()));
                }
            }
            let reply = model
                .invoke(&messages, &call_options)
                .await
                .map_err(|e| rustakka_langgraph_core::errors::GraphError::other(e.to_string()))?;
            serde_json::to_value(reply)
                .map_err(|e| rustakka_langgraph_core::errors::GraphError::other(e.to_string()))
        })
    })
}

/// Convenience view of the compiled react agent — stable across
/// future refactors of `PersonaReactAgent`'s field layout. Callers
/// that just want to introspect the graph (mermaid, blueprint asserts,
/// feeding a runtime) can accept `&dyn ReactAgentLike` instead of a
/// concrete type.
pub trait ReactAgentLike: Send + Sync {
    fn blueprint(&self) -> &Blueprint;
    fn compiled(&self) -> &Arc<rustakka_langgraph_core::graph::CompiledStateGraph>;
}

impl ReactAgentLike for PersonaReactAgent {
    fn blueprint(&self) -> &Blueprint {
        &self.graph.blueprint
    }
    fn compiled(&self) -> &Arc<rustakka_langgraph_core::graph::CompiledStateGraph> {
        &self.graph.compiled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::mock::echo_provider;
    use rustakka_agent_eq::{EqProfile, Reflection};
    use rustakka_agent_iq::IqProfile;

    fn model() -> Arc<dyn ProviderChatModel> {
        echo_provider("echo")
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn persona_none_is_behaviorally_identical_to_bare_react() {
        let opts = AgentOptions {
            persona: None,
            react: ReactAgentOptions {
                system_prompt: Some("you are helpful".into()),
                ..ReactAgentOptions::default()
            },
        };
        let a = create_persona_react_agent(model(), vec![], opts).await.unwrap();
        assert_eq!(
            a.graph.blueprint.system_prompt.as_deref(),
            Some("you are helpful")
        );
        assert!(a.graph.blueprint.recursion_limit.is_none());
        assert!(a.graph.compiled.topology().nodes.contains_key("agent"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn persona_prompt_prepends_with_user_overrides_appended() {
        let persona = Persona::builder().name("Ada").role("tutor").build();
        let opts = AgentOptions {
            persona: Some(persona),
            react: ReactAgentOptions {
                system_prompt: Some("speak only in haiku".into()),
                ..ReactAgentOptions::default()
            },
        };
        let a = create_persona_react_agent(model(), vec![], opts).await.unwrap();
        let sp = a.graph.blueprint.system_prompt.as_ref().unwrap();
        assert!(sp.contains("Ada"));
        assert!(sp.contains("[User overrides]"));
        assert!(sp.contains("speak only in haiku"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn persona_iq_folds_into_call_options_and_recursion_limit() {
        let iq = IqProfile::builder()
            .reasoning_depth(0.6)
            .planning_hops(4)
            .temperature(0.2)
            .preferred_model("gpt-4o")
            .verbosity(0.5)
            .build();
        let persona = Persona::builder().name("Pat").iq(iq).build();
        let opts = AgentOptions {
            persona: Some(persona),
            react: ReactAgentOptions::default(),
        };
        let a = create_persona_react_agent(model(), vec![], opts).await.unwrap();
        assert_eq!(a.graph.call_options.temperature, Some(0.2));
        assert!(a.graph.call_options.max_tokens.is_some());
        assert!(a.graph.blueprint.recursion_limit.is_some());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn caller_pinned_recursion_limit_wins() {
        let iq = IqProfile::builder().planning_hops(10).build();
        let persona = Persona::builder().iq(iq).build();
        let opts = AgentOptions {
            persona: Some(persona),
            react: ReactAgentOptions {
                recursion_limit: Some(3),
                ..ReactAgentOptions::default()
            },
        };
        let a = create_persona_react_agent(model(), vec![], opts).await.unwrap();
        assert_eq!(a.graph.blueprint.recursion_limit, Some(3));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn reflection_node_inserted_when_eq_requests_after_each_turn() {
        let eq = EqProfile::builder()
            .reflection(Reflection::AfterEachTurn)
            .build();
        let persona = Persona::builder().eq(eq).build();
        let a = create_persona_react_agent(
            model(),
            vec![],
            AgentOptions {
                persona: Some(persona),
                react: ReactAgentOptions::default(),
            },
        )
        .await
        .unwrap();
        assert!(a.graph.blueprint.has_node("reflect"));
        assert!(a.graph.blueprint.has_edge("agent", "reflect"));
    }
}

