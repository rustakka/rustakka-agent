# Oh-My-OpenAgent-style harness graph

Source: [`rustakka-agent-prebuilt::omo_harness`](../crates/rustakka-agent-prebuilt/src/omo_harness/).
Cargo feature: `omo-harness` (enabled by default on the umbrella
crate).

Modeled after
[Oh My OpenAgent](https://github.com/code-yeongyu/oh-my-openagent): a
hierarchical, category-routed multi-agent orchestrator. We re-express
the *pattern* in `rustakka-agent` terms — one orchestrator persona,
a set of discipline personas, category routing, and optional edit
safety.

## Topology

```
                       ┌────────────┐
user ──▶ IntentGate ──▶│  sisyphus  │◀── returns here each hop
                       └────┬───────┘
                            │ category routing (persona-aware)
     ┌─────────┬─────────┬──┼──┬─────────┬─────────┐
     ▼         ▼         ▼  ▼  ▼         ▼         ▼
 prometheus hephaestus oracle librarian explore  visio   quick
     │         │         │    │         │         │       │
     └─────────┴─────────┴────┴─────────┴─────────┴───────┘
                            ▼
                       BoulderStore   (session continuity, channel `omo.boulder`)
                            ▼
                       HashlineGate   (edit-safety middleware, Off | Warn | Enforce)
                            ▼
                          END
```

Under the hood, `create_omo_harness` calls
[`create_persona_supervisor`](../crates/rustakka-agent-prebuilt/src/supervisor.rs)
with a `PersonaAware` router, then layers in the boulder channel
(`omo.boulder`) and a hashline-mode annotation
(`omo.hashline.mode.<Enforce|Warn|Off>`).

## Canonical disciplines

Shipped by `default_disciplines(ladder, model)`. Every discipline is
a `PersonaAgent` compiled from a persona that pins the listed IQ tier
and declares the listed category.

| Discipline    | Default tier | Default category          |
|---------------|--------------|---------------------------|
| `sisyphus`    | `Strategist` | `orchestration`           |
| `prometheus`  | `Strategist` | `planning`                |
| `hephaestus`  | `Scholar`    | `deep`                    |
| `oracle`      | `Strategist` | `ultrabrain`              |
| `librarian`   | `Analyst`    | `documentation`           |
| `explore`     | `Operator`   | `search`                  |
| `visio`       | `Analyst`    | `visual-engineering`      |
| `quick`       | `Reflex`     | `quick`                   |

Callers can filter or replace the list before passing it to
`create_omo_harness`.

## Options

```rust
pub struct OmoHarnessOptions {
    pub ladder: IqLadder,
    pub orchestrator: PersonaAgent,
    pub disciplines: Vec<PersonaAgent>,
    pub boulder_store: Option<Arc<dyn BaseStore>>,
    pub hashline: HashlineMode, // Off | Warn | Enforce
    pub default_set: bool,
}
```

`BaseStore` is a small `get` / `set` interface provided by
`rustakka-langgraph-store` and re-exported from
`rustakka_agent::prebuilt::graph::{BaseStore, InMemoryStore}`. The
default in-memory impl `InMemoryStore` is fine for dev and test;
prod deployments typically wire a Postgres-backed store here.

After compile, the supplied store is forwarded onto the returned
`AgentGraph` and can be handed to the upstream runner directly:

```rust,ignore
use rustakka_langgraph_core::runner::invoke_with_store;

let accessor = app.graph.store_accessor().expect("boulder_store was Some");
let result = invoke_with_store(
    app.graph.compiled.clone(),
    input,
    cfg,
    None,         // no checkpointer
    accessor,
).await?;
```

## Usage

```rust,ignore
let ladder = IqLadder::builder()
    .tier(IqTier::Scholar, openai_rung("gpt-4o"))
    .tier(IqTier::Strategist, openai_rung("gpt-4o"))
    .tier(IqTier::Analyst, openai_rung("gpt-4o-mini"))
    .tier(IqTier::Operator, openai_rung("gpt-4o-mini"))
    .tier(IqTier::Reflex, ollama_rung("llama3:8b"))
    .build();

let model = provider.gpt4o();
let app = create_omo_harness(OmoHarnessOptions {
    ladder: ladder.clone(),
    orchestrator: sisyphus(&ladder, model.clone()).await?,
    disciplines: default_disciplines(&ladder, model.clone()).await?,
    boulder_store: Some(Arc::new(InMemoryStore::new())),
    hashline: HashlineMode::Enforce,
    default_set: true,
}).await?;
```

## Non-goals

- We do not reproduce Oh-My-OpenAgent's prompt text verbatim — only
  the topology, state, and routing semantics. Prompt content is
  persona-rendered.
- We do not bundle vendor-specific MCP servers; MCP tools plug in via
  the upstream `Tool` registration path.
