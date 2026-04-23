# rustakka-agent

`rustakka-agent` is an extension of
[`rustakka-langgraph`](https://github.com/rustakka/rustakka-langgraph)
that layers a **first-class agent-characteristics model** on top of the
LangGraph-compatible Pregel engine.

Where `rustakka-langgraph` gives you *the mechanics* of an agentic
graph — nodes, channels, checkpointers, streaming, ReAct / Supervisor /
Swarm prebuilts — `rustakka-agent` gives you *the personality* of the
agents that run inside those graphs:

- **IQ** — cognitive profile (reasoning depth, planning hops, tool-use
  aggressiveness, verbosity, model/temperature selection, tier ladder).
- **EQ** — emotional profile (empathy, tone, mood, reflection cadence).
- **Persona** — an optional, strongly-typed bundle of *non-physical*
  characteristics: identity, role, values, goals, communication style,
  taboos, knowledge domains, memory preferences, and safety rails.

Personas compile down to:

1. a deterministic **system-prompt fragment** injected into any ReAct /
   Supervisor / Swarm graph,
2. a set of **`CallOptions`** tweaks (temperature, top-p, max tokens,
   model selection via the `IqLadder`), and
3. a set of **graph knobs** (recursion limit, reflection-node
   injection, tool biasing) applied at compile time.

`rustakka-agent` is an **additive layer** on top of
`rustakka-langgraph`. `rustakka-agent-prebuilt` depends directly on
the upstream crates (`rustakka-langgraph-core`,
`rustakka-langgraph-providers`, `rustakka-langgraph-prebuilt`,
`rustakka-langgraph-store`) and returns real
[`rustakka_langgraph_core::graph::CompiledStateGraph`]s — the same
values any upstream caller would obtain from `create_react_agent`,
`create_supervisor`, or `create_swarm`. The only new runtime types
are:

- [`Blueprint`](crates/rustakka-agent-prebuilt/src/graph.rs) — a
  serializable topology description (nodes, edges, channels, system
  prompt, recursion limit, interrupt points). Every persona-aware
  builder produces one, so tests can assert structure without
  executing the graph.
- [`AgentGraph`](crates/rustakka-agent-prebuilt/src/graph.rs) — pairs
  a `Blueprint` with the compiled upstream graph, the
  `CallOptions`, the `Tool` list, and the `ChatModel` that actually
  runs it.
- A `langgraph` feature on
  [`rustakka-agent-iq`](crates/rustakka-agent-iq) that provides
  blanket `CallOptionsLike` / `ChatModel` impls for the upstream
  provider types, so `IqLadder` and `IqCarryings` talk directly to
  `rustakka-langgraph-providers::CallOptions` at runtime while
  `rustakka-agent-iq` remains compilable on its own.

## Workspace layout

```
crates/
├─ rustakka-agent-traits     # Trait / Score / Dimension / TraitSet / AgentEnv
├─ rustakka-agent-iq         # IqProfile, IqTier, IqLadder, carryings
├─ rustakka-agent-eq         # EqProfile, Mood, Reflection cadence
├─ rustakka-agent-persona    # Persona bundle + loaders + validation
├─ rustakka-agent-prebuilt   # create_persona_react_agent / supervisor / swarm
│                              + patterns (15) + aiq_research + omo_harness
├─ rustakka-agent-profiler   # micro-bench scenarios
└─ rustakka-agent            # umbrella facade, feature-gated re-exports
```

## Feature flags

The umbrella crate ships the full stack by default. Turn features off
to slim a build:

| Feature        | What it enables                                                            |
|----------------|----------------------------------------------------------------------------|
| `persona`      | `Persona`, `Identity`, loaders (`json`, `yaml`, `toml`), validators.       |
| `prebuilt`     | `create_persona_react_agent`, supervisor & swarm, reflection, tool bias.   |
| `patterns`     | 15-pattern catalog (plan-execute, reflexion, RAG, CRAG, …).                |
| `aiq-research` | AI-Q-style deep-research graph.                                            |
| `omo-harness`  | Oh-My-OpenAgent-style harness graph.                                       |

## Quick start

```rust,ignore
use rustakka_agent::prelude::*;

let persona = Persona::builder()
    .name("Ada")
    .role("math tutor")
    .values(["clarity", "accuracy"])
    .iq(IqProfile::builder()
        .reasoning_depth(0.7)
        .planning_hops(3)
        .preferred_model("gpt-4o")
        .temperature(0.3)
        .build())
    .eq(EqProfile::builder()
        .empathy(0.8)
        .warmth(0.7)
        .reflection(Reflection::OnError)
        .build())
    .build();

let agent = create_persona_react_agent(
    model,
    tools,
    AgentOptions {
        persona: Some(persona),
        react: ReactAgentOptions::default(),
    },
).await?;
```

See the runnable examples in
[`crates/rustakka-agent/examples/`](crates/rustakka-agent/examples):

```bash
cargo run -p rustakka-agent --example rust_persona_react
cargo run -p rustakka-agent --example rust_supervisor_team
cargo run -p rustakka-agent --example rust_aiq_research
cargo run -p rustakka-agent --example rust_omo_harness
cargo run -p rustakka-agent --example rust_pattern_plan_execute
cargo run -p rustakka-agent --example rust_pattern_reflexion
cargo run -p rustakka-agent --example rust_pattern_rag_suite
cargo run -p rustakka-agent --example rust_pattern_debate
```

## Documentation

- [`docs/plan.md`](docs/plan.md) — full design, non-goals, risks.
- [`docs/TODO.md`](docs/TODO.md) — phase-by-phase checklist.
- [`docs/persona-schema.md`](docs/persona-schema.md) — JSON/YAML/TOML
  persona schema, loaders, validation.
- [`docs/iq-ladders.md`](docs/iq-ladders.md) — tier model, carryings,
  external ladder format.
- [`docs/patterns.md`](docs/patterns.md) — 15-pattern catalog and
  composition rules.
- [`docs/aiq-research.md`](docs/aiq-research.md) — deep-research graph.
- [`docs/omo-harness.md`](docs/omo-harness.md) — harness graph.
- [`docs/integration.md`](docs/integration.md) — migrating an existing
  `create_react_agent` caller.

## Status

All nine Rust-first phases are complete and the whole workspace now
builds directly against `rustakka-langgraph` (no mock seam):

- `cargo check --workspace --all-targets`
- `cargo test --workspace --all-targets` (89+ tests, all green)
- `cargo clippy --workspace --all-targets -- -D warnings`

`rustakka-agent-prebuilt` assumes `rustakka-langgraph` is available
as a **sibling checkout** at `../rustakka-langgraph`, and that the
`rustakka` actor runtime is at `../rustakka`. Both are wired through
the root `Cargo.toml`:

- `[workspace.dependencies]` points the `rustakka-langgraph-*` crates
  at the local sibling.
- `[patch."https://github.com/cognect/rustakka"]` mirrors the same
  `[patch]` section from upstream `rustakka-langgraph`, redirecting
  the pinned git `rustakka-streams` (and friends) to the local
  sibling checkout. Without this, upstream fails to find newer
  `rustakka-streams` APIs (`Source::from_receiver`, `KillSwitch`, …)
  that power the latest concurrent `ToolNode` and streaming paths.

Additional surface exposed from `rustakka-agent-prebuilt::graph` to
match recent upstream additions:

- `ToolNodeOptions` — bounded-parallelism tool execution (upstream
  default `parallelism = 8`).
- `chat_model_stream_source` — wrap any `Arc<dyn ChatModel>` as a
  `rustakka_streams::Source` that can be composed with upstream
  operators.
- `BaseStore`, `InMemoryStore`, `StoreAccessor`, `store_accessor` —
  attach an `AgentGraph` to `invoke_with_store`. `omo_harness`
  forwards its `boulder_store` onto the returned graph, and
  `AgentGraph::store_accessor()` yields a ready-to-use accessor.

Python bindings (Phase 7) are intentionally deferred.

## License

Dual-licensed MIT / Apache-2.0 at your option.
