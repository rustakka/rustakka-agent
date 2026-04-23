# Integrating `rustakka-agent` into an existing `create_react_agent` caller

This guide walks through migrating an existing
[`rustakka-langgraph`](https://github.com/rustakka/rustakka-langgraph)
project to use `rustakka-agent`'s persona-aware wrappers. The
refactor is **additive**: `persona=None` produces a graph that is
byte-for-byte equivalent to the upstream prebuilt.

## 0. Dependencies

`rustakka-agent-prebuilt` depends directly on the upstream crates,
so enabling the `prebuilt` feature automatically pulls
`rustakka-langgraph-core`, `rustakka-langgraph-providers`,
`rustakka-langgraph-prebuilt`, and `rustakka-langgraph-store`:

```toml
# Cargo.toml
[dependencies]
rustakka-agent = { version = "0.1", features = ["prebuilt"] }
```

The umbrella crate re-exports the upstream provider traits under
`rustakka_agent::providers::*` so most callers don't need to add
`rustakka-langgraph-providers` explicitly. If you want to hand a
`CompiledStateGraph` straight to a runner you already own, pull in
`rustakka-langgraph-core` as usual.

The `rustakka-agent-prebuilt::graph` module also re-exports a few
companion surfaces that track upstream's latest feature set:

| Upstream item                                          | Exposed as                                   |
|--------------------------------------------------------|----------------------------------------------|
| `rustakka_langgraph_prebuilt::tool_node::ToolNodeOptions` | `graph::ToolNodeOptions`                    |
| `rustakka_langgraph_providers::traits::chat_model_stream_source` | `graph::chat_model_stream_source`    |
| `rustakka_langgraph_core::context::StoreAccessor`      | `graph::StoreAccessor`                       |
| `rustakka_langgraph_store::{BaseStore, InMemoryStore, store_accessor}` | `graph::{BaseStore, InMemoryStore, store_accessor}` |

`AgentGraph` gained a `store: Option<Arc<dyn BaseStore>>` field and a
`store_accessor()` helper. When `omo_harness` is given a
`boulder_store` it is placed on the returned graph, so downstream
runners can call
`rustakka_langgraph_core::runner::invoke_with_store(agent.graph.compiled.clone(), input, cfg, ckpt, agent.graph.store_accessor().unwrap())`
without threading the store separately.

Feature flags:

| Flag           | Pulls in                                         |
|----------------|--------------------------------------------------|
| `persona`      | `Persona`, loaders, validators                   |
| `prebuilt`     | `create_persona_react_agent` + supervisor/swarm  |
| `patterns`     | 15-pattern catalog                               |
| `aiq-research` | Deep-research graph                              |
| `omo-harness`  | Harness graph                                    |

## 1. Direct drop-in

Before:

```rust,ignore
use rustakka_langgraph_prebuilt::react_agent::{create_react_agent, ReactAgentOptions};

let app = create_react_agent(
    model_fn,
    tools,
    ReactAgentOptions {
        system_prompt: Some("you are helpful".into()),
        ..ReactAgentOptions::default()
    },
).await?;
```

After (no persona):

```rust,ignore
use rustakka_agent::prelude::*;

let agent = create_persona_react_agent(
    model,           // Arc<dyn ChatModel> from rustakka-langgraph-providers
    tools,           // Vec<rustakka_agent::prebuilt::graph::Tool>
    AgentOptions {
        persona: None,
        react: ReactAgentOptions {
            system_prompt: Some("you are helpful".into()),
            ..ReactAgentOptions::default()
        },
    },
).await?;

// agent.graph.compiled is the real upstream CompiledStateGraph;
// agent.graph.blueprint describes the topology for tests/mermaid.
```

By construction, `persona = None` behaves identically to the upstream
call (parity test in `rustakka-agent-prebuilt::react::tests`).

## 2. Layer a persona

```rust,ignore
let persona = Persona::builder()
    .name("Ada")
    .role("tutor")
    .iq(IqProfile::builder()
        .reasoning_depth(0.7)
        .planning_hops(3)
        .preferred_model("gpt-4o")
        .temperature(0.3)
        .build())
    .values(["clarity", "accuracy"])
    .build();

let app = create_persona_react_agent(
    model,
    tools,
    AgentOptions {
        persona: Some(persona),
        react: ReactAgentOptions::default(),
    },
).await?;
```

What changes:

- The persona's `to_system_prompt()` is merged into
  `react.system_prompt` (persona wins; user prompt is appended under
  `[User overrides]`).
- `iq.temperature`, `iq.preferred_model`, and a verbosity-derived
  `max_tokens` cap are folded into `react.call_options`
  (`top_p` lands in `CallOptions.extra["top_p"]`, matching the
  upstream provider convention).
- `iq.recommended_recursion_limit()` populates
  `compile.recursion_limit` *iff the caller hasn't set one already*.
- `eq.reflection_cadence` may insert a `reflect` node into the graph
  (see [`inject_reflection`](../crates/rustakka-agent-prebuilt/src/reflection.rs)).
- `iq.tool_eagerness` writes a `router.tool_bias` or
  `router.suppress_tools` channel that the upstream engine adapter
  reads when wrapping `tools_condition`.

## 3. Supervisor / swarm

```rust,ignore
let boss = PersonaAgent::new(
    "boss", model.clone(), boss_persona, vec![], vec!["orchestration".into()]
).await?;
let a = PersonaAgent::new("a", model.clone(), persona_a, tools_a, vec!["reasoning".into()]).await?;
let b = PersonaAgent::new("b", model.clone(), persona_b, tools_b, vec!["writing".into()]).await?;

let app = create_persona_supervisor(boss, SupervisorRouter::PersonaAware, vec![a, b]).await?;
// or fully-connected:
// let app = create_persona_swarm(vec![a, b]).await?;
```

`persona_based_router(&agents, hint)` is exposed so callers can reuse
the routing logic outside a compiled graph — handy for custom routing
nodes or unit tests.

## 4. Environment awareness

All crates read `RUSTAKKA_AGENT_ENV` (default: `dev`):

| Env    | Default provider        | Logging       | Safety rails | Persona validation |
|--------|-------------------------|---------------|--------------|--------------------|
| `dev`  | Real provider if keys   | `debug`       | relaxed      | warn on conflicts  |
| `test` | `MockChatModel` always  | `info`, JSON  | strictest    | fail on conflicts  |
| `prod` | Real provider required  | `info`        | strict       | fail on conflicts  |

When `AgentEnv::Test` is active, `IqLadder::select` returns a
deterministic `MockChatModel` rung regardless of what the ladder says.
This keeps snapshot tests stable across machines.

## 5. Where to go next

- [persona-schema.md](persona-schema.md) — every field in a persona.
- [iq-ladders.md](iq-ladders.md) — tiers, carryings, ladder loaders.
- [patterns.md](patterns.md) — 15-pattern catalog and composition rules.
- [aiq-research.md](aiq-research.md) — deep-research graph.
- [omo-harness.md](omo-harness.md) — multi-persona harness.
