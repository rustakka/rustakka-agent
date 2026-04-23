# rustakka-agent — Implementation Plan

This document specifies **how** we extend
[`rustakka-langgraph`](https://github.com/rustakka/rustakka-langgraph)
into a reusable Rust agent library that:

1. defines an **agent-characteristics** model (IQ / EQ / traits),
2. supports optional **personas** bundling non-physical attributes
   (identity, role, communication style, values, goals, …), and
3. weaves both of the above non-invasively into the existing
   LangGraph-compatible graph compilation path
   (`create_react_agent`, `create_supervisor`, `create_swarm`).

It follows the same conventions as `rustakka-langgraph`: a Cargo
workspace of focused crates, a feature-gated umbrella facade, an
optional `pyo3` / `maturin` Python shim, and phase-by-phase progress
tracked in [`docs/TODO.md`](TODO.md).

---

## 1. Goals and non-goals

### Goals

- **Compose, don't fork.** `rustakka-agent` depends on
  `rustakka-langgraph` as a normal crate. No patching of upstream
  internals.
- **Typed personality.** Personalities, IQ, EQ, and personas are
  `#[derive(Serialize, Deserialize, Clone, Debug)]` structs with
  builder APIs. They round-trip to/from JSON, YAML, and TOML.
- **Declarative → executable.** A `Persona` + a `ChatModel` (from
  `rustakka-langgraph-providers`) + a tool-set compiles to a
  `CompiledStateGraph` via the existing prebuilt factories.
- **Dev / test / prod aware.** Every crate honors
  `RUSTAKKA_AGENT_ENV ∈ {dev,test,prod}` (mirroring
  `RUSTAKKA_LANGGRAPH_ENV`) for logging, tracing, default safety
  rails, and mock-provider selection.
- **Python parity.** A thin `rustakka_agent` Python module mirrors the
  Rust surface and plugs into the existing `langgraph` façade, so
  `create_react_agent(..., persona=...)` works from Python.

### Non-goals

- Physical avatars, voice synthesis, or rendering. Personas describe
  *non-physical* characteristics only.
- A new LLM provider stack — we reuse
  `rustakka-langgraph-providers`.
- Breaking changes to `rustakka-langgraph`. All integration points
  are additive.

---

## 2. High-level architecture

```text
         ┌───────────────────────────────────────────────┐
         │                 rustakka-agent                │  (umbrella)
         └───────────────────────────────────────────────┘
           │         │          │            │          │
           ▼         ▼          ▼            ▼          ▼
   ┌──────────┐ ┌────────┐ ┌──────────┐ ┌──────────┐ ┌────────────┐
   │  traits  │ │  iq    │ │   eq     │ │ persona  │ │  prebuilt  │
   │  (core)  │ │        │ │          │ │          │ │  adapter   │
   └──────────┘ └────────┘ └──────────┘ └──────────┘ └────────────┘
         │          │           │            │             │
         └──────────┴───────────┴────────────┘             │
                             │                              │
                             ▼                              ▼
                    ┌─────────────────┐          ┌────────────────────┐
                    │ rustakka-       │          │ rustakka-langgraph │
                    │ langgraph-      │◀─────────│  -prebuilt / core /│
                    │ providers       │          │  providers         │
                    │ (ChatModel)     │          └────────────────────┘
                    └─────────────────┘
```

The **only** crate that physically depends on
`rustakka-langgraph-prebuilt` is `rustakka-agent-prebuilt`; the trait /
iq / eq / persona crates are pure data + logic and stay decoupled.

---

## 3. Workspace layout

```text
crates/
  rustakka-agent-traits/       # Trait + Score + Dimension primitives
  rustakka-agent-iq/           # IQ profile + IqTier + IqLadder / carryings
  rustakka-agent-eq/           # EQ profile: empathy, tone, mood, reflection
  rustakka-agent-persona/      # Persona struct + builder + (de)serialization
  rustakka-agent-prebuilt/     # Bridge into rustakka-langgraph prebuilts
                               # + aiq_research + omo_harness graphs
                               # + patterns catalog (plan-execute,
                               #   reflexion, evaluator-optimizer,
                               #   self-consistency, tot/lats, debate,
                               #   router/MoE, rag, crag, adaptive-rag,
                               #   self-rag, hitl gate, memory agent,
                               #   codex loop, guardrails)
  rustakka-agent/              # Umbrella façade (feature-gated re-exports)
  rustakka-agent-profiler/     # Micro-bench: persona-compile / prompt-assembly
  py-bindings/pyagent/         # Optional PyO3 cdylib -> rustakka_agent._native
python/
  rustakka_agent/              # Python package wrapping the cdylib
  tests/                       # pytest parity suite
examples/
  rust_persona_react/          # ReAct agent with a persona
  rust_supervisor_team/        # Multi-persona supervisor team
  rust_aiq_research/           # AI-Q-style deep-research graph
  rust_omo_harness/            # Oh-My-OpenAgent-style harness graph
  rust_pattern_plan_execute/   # Plan-and-Execute pattern
  rust_pattern_reflexion/      # Reflexion pattern
  rust_pattern_rag_suite/      # RAG + CRAG + Adaptive-RAG + Self-RAG
  rust_pattern_debate/         # Debate / jury pattern
docs/
  plan.md                      # (this file)
  TODO.md
  persona-schema.md
  iq-ladders.md                # YAML/TOML schema for IqLadder / carryings
  aiq-research.md              # AI-Q graph topology, state, middleware
  omo-harness.md               # OMO harness topology, boulder, hashline
  patterns.md                  # Catalog of agentic patterns + composition
  integration.md
```

This shape mirrors `rustakka-langgraph` 1:1, which keeps mental model
cost low for contributors working across both repos.

---

## 4. Core data model

### 4.1 `Trait` (crate: `rustakka-agent-traits`)

`Trait` is the atomic building block. It is deliberately small so that
IQ, EQ, and Persona can all be expressed as typed bundles of traits.

```rust
/// A bounded, non-physical characteristic with a numeric magnitude.
/// Scores are normalized to `[0.0, 1.0]`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Trait {
    pub name: String,            // e.g. "curiosity", "empathy"
    pub score: Score,            // 0.0..=1.0
    pub dimension: Dimension,    // IQ | EQ | Style | Values | Safety | Custom
    pub notes: Option<String>,   // free-form guidance for the LLM
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Score(f32);           // clamped at construction

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Dimension { Iq, Eq, Style, Values, Safety, Custom }
```

A `TraitSet` is `BTreeMap<String, Trait>` with convenience impls
(`merge`, `with`, `without`, `to_prompt_fragment`).

### 4.2 IQ profile (crate: `rustakka-agent-iq`)

```rust
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct IqProfile {
    pub reasoning_depth: Score,      // chain-of-thought richness
    pub planning_hops: u32,          // maps to recursion_limit nudges
    pub tool_eagerness: Score,       // controls tools_condition bias
    pub verbosity: Score,            // maps to max_tokens / brevity hints
    pub preferred_model: Option<String>, // e.g. "gpt-4o-mini"
    pub temperature: Option<f32>,
    pub extra: TraitSet,             // user-defined cognitive traits
}
```

Effects at compile time:

- `planning_hops` contributes to `ReactAgentOptions.recursion_limit`.
- `tool_eagerness` influences `tools_condition` bias (added via a
  wrapping router in `rustakka-agent-prebuilt`).
- `temperature` / `preferred_model` are folded into `CallOptions`
  when a `ChatModel` is bound.
- `verbosity` adds a prompt fragment like
  `"Be concise: target {n} sentences per reply."`.

#### 4.2.1 IQ tiers & per-tier model ladders

An `IqProfile` alone does not pin a specific LLM; instead we bucket
profiles into **IQ tiers** and let each tier carry its own *ladder* of
models and call-time carryings (temperature, top-p, max-tokens,
context window, tool allow-list, cache policy, …). Callers can then
bind a single `ChatModel` *ladder* to a persona and let the profile
choose which rung to use at runtime.

```rust
/// Coarse IQ bucket. Ranges are expressed in terms of a composite
/// score `w_depth * reasoning_depth + w_plan * normalize(planning_hops)
/// + w_tool * tool_eagerness`, clamped to [0, 1].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IqTier {
    /// 0.00..0.20 — tiny, reactive agents (FAQ, classifier-style).
    Reflex,
    /// 0.20..0.40 — bounded tool loops, single-hop research.
    Operator,
    /// 0.40..0.60 — general assistant, 2–3 hop planning.
    Analyst,
    /// 0.60..0.80 — multi-step planning, self-critique, tool teams.
    Strategist,
    /// 0.80..1.00 — deep research, long-horizon, ensemble reasoning.
    Scholar,
}

/// Call-time "carryings" — runtime knobs applied to every LLM call
/// made by a node running under this tier.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IqCarryings {
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_output_tokens: Option<u32>,
    pub context_window_hint: Option<u32>,
    pub recursion_limit: Option<u32>,
    pub cache_policy: Option<CachePolicy>,       // re-exported upstream
    pub tool_allow_list: Option<Vec<String>>,
    pub system_prompt_addendum: Option<String>,
}

/// A single rung on a tier's model ladder. Rungs are tried top-to-
/// bottom; the first one whose `predicate` (if any) accepts the
/// bound `ChatModel` + `CallOptions` is used.
#[derive(Clone)]
pub struct ModelRung {
    pub name: String,                             // e.g. "gpt-4o"
    pub model: Arc<dyn ChatModel>,                // from providers crate
    pub carryings: IqCarryings,
    pub predicate: Option<Arc<dyn Fn(&IqProfile) -> bool + Send + Sync>>,
}

/// Ordered ladder of model rungs for a single tier.
#[derive(Clone)]
pub struct TierLadder {
    pub tier: IqTier,
    pub rungs: Vec<ModelRung>,
}

/// Full ladder across every tier. Missing tiers fall back to the
/// next-higher defined tier, then to `default` (if set).
#[derive(Clone, Default)]
pub struct IqLadder {
    pub tiers: BTreeMap<IqTier, TierLadder>,
    pub default: Option<ModelRung>,
}

impl IqLadder {
    pub fn builder() -> IqLadderBuilder { /* … */ }

    /// Select a rung for the given profile.
    pub fn select(&self, iq: &IqProfile) -> Option<&ModelRung> { /* … */ }

    /// Fold the selected rung's `IqCarryings` into `CallOptions`.
    pub fn apply(&self, iq: &IqProfile, opts: &mut CallOptions) { /* … */ }
}
```

Recommended **default ladder** (users override freely; choices are
examples, not a hard dependency):

| Tier         | Score    | Typical carryings                                    | Example model rungs (top→bottom fallback)                     |
|--------------|----------|------------------------------------------------------|---------------------------------------------------------------|
| `Reflex`     | 0.00–0.20 | `temperature=0.0`, `max_output_tokens≈256`, no tools | `gpt-4o-mini` → `llama3:8b` → `mock`                           |
| `Operator`   | 0.20–0.40 | `temperature=0.2`, `max_output_tokens≈768`, curated tools | `gpt-4o-mini` → `claude-haiku` → `llama3:8b`            |
| `Analyst`    | 0.40–0.60 | `temperature=0.3`, `max_output_tokens≈2048`, tools on | `gpt-4o` → `claude-sonnet` → `llama3:70b`                    |
| `Strategist` | 0.60–0.80 | `temperature=0.4`, `max_output_tokens≈4096`, reflection on | `gpt-4o` → `claude-sonnet-thinking` → `nemotron-70b`    |
| `Scholar`    | 0.80–1.00 | `temperature=0.5`, `max_output_tokens≈8192`, ensemble + deep-research graph | `nemotron-ultra-253b` → `gpt-4o` → `claude-opus`|

Additional design points:

- **Ladder rungs are first-class.** `ModelRung.predicate` lets a
  ladder react to more than just tier — e.g. "if the persona
  pins `preferred_model="gpt-4o"`, prefer the `gpt-4o` rung".
- **Carryings compose additively.** `IqCarryings` is folded in the
  order: *ladder default* → *tier carryings* → *rung carryings* →
  *persona overrides* → *caller overrides*. Later values win.
- **Test/dev determinism.** When `AgentEnv::current() == Test`, the
  ladder resolver forces `MockChatModel` regardless of rung, so
  snapshot tests remain deterministic.
- **Tier inference.** `IqProfile::tier()` returns the natural bucket
  for the composite score; users can pin a tier explicitly via
  `IqProfile.pinned_tier: Option<IqTier>`.

See `docs/iq-ladders.md` (authored in Phase 1b) for the authoritative
YAML/TOML schema for external ladder definitions.

### 4.3 EQ profile (crate: `rustakka-agent-eq`)

```rust
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct EqProfile {
    pub empathy: Score,
    pub warmth: Score,
    pub assertiveness: Score,
    pub humor: Score,
    pub mood: Mood,                     // Neutral | Upbeat | Calm | Serious | …
    pub reflection_cadence: Reflection, // Never | AfterEachTurn | OnError | …
    pub extra: TraitSet,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum Mood { #[default] Neutral, Upbeat, Calm, Serious, Playful, Stoic }

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum Reflection { #[default] Never, AfterEachTurn, OnError, OnToolFailure }
```

Effects at compile time:

- `reflection_cadence` may inject a `reflect` node between
  `agent → tools` / `tools → agent` or after terminal turns.
- `empathy`, `warmth`, `humor` produce prompt fragments that modulate
  tone guidance.
- `mood` maps to a small library of canonical tone directives.

### 4.4 Persona (crate: `rustakka-agent-persona`)

```rust
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Persona {
    pub identity: Identity,          // name, pronouns, role, bio
    pub iq: IqProfile,
    pub eq: EqProfile,
    pub values: Vec<String>,         // short imperative phrases
    pub goals: Vec<String>,          // agent's standing objectives
    pub style: CommunicationStyle,   // register, formality, language
    pub knowledge_domains: Vec<String>,
    pub taboos: Vec<String>,         // hard "do not" rules
    pub memory: MemoryPrefs,         // long-term-memory strategy hints
    pub safety: SafetyRails,         // refusal style, red-lines
    pub custom: TraitSet,            // free-form extensions
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Identity {
    pub name: Option<String>,
    pub pronouns: Option<String>,
    pub role: Option<String>,        // "triage analyst", "tutor", …
    pub bio: Option<String>,         // 1–3 sentence backstory
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct CommunicationStyle {
    pub formality: Score,            // 0 = casual, 1 = formal
    pub register: Register,          // Plain | Technical | Socratic | …
    pub language: Option<String>,    // BCP-47; default = "en"
    pub signature_phrases: Vec<String>,
}
```

`Persona` implements:

- `builder()` (typed, chainable).
- `from_toml` / `from_yaml` / `from_json` (loader helpers with
  env-aware defaults; e.g. test-env personas auto-enable a
  deterministic mock provider).
- `to_system_prompt(&self) -> String` — canonical, deterministic
  rendering suitable for `ReactAgentOptions.system_prompt`.
- `apply_to_call_options(&self, opts: &mut CallOptions)` — hoists
  `iq.temperature` / `iq.preferred_model` / verbosity caps.
- `validate(&self) -> Result<(), PersonaError>` — bounds-check
  scores, conflict-check (e.g. `safety.deny_all = true` must not
  coexist with `taboos` exceptions).

`Persona` is **optional** everywhere: any API that takes a persona
also works with `Option<Persona>`, and `None` is equivalent to the
current `rustakka-langgraph` behavior.

---

## 5. Integration with `rustakka-langgraph`

Crate: `rustakka-agent-prebuilt`. This is the only crate that pulls in
`rustakka-langgraph-prebuilt` and `rustakka-langgraph-providers`. It
exposes **thin, additive** wrappers:

```rust
pub struct AgentOptions {
    pub persona: Option<Persona>,
    pub react: ReactAgentOptions,     // upstream options, unchanged
}

pub async fn create_persona_react_agent(
    model: Arc<dyn ChatModel>,
    tools: Vec<Tool>,
    opts: AgentOptions,
) -> GraphResult<CompiledStateGraph>;

pub async fn create_persona_supervisor(
    supervisor: PersonaAgent,          // Persona + NodeKind
    router: SupervisorRouter,
    agents: Vec<PersonaAgent>,
) -> GraphResult<CompiledStateGraph>;

pub async fn create_persona_swarm(
    agents: Vec<PersonaAgent>,
) -> GraphResult<CompiledStateGraph>;
```

Internally:

1. Persona → `system_prompt`
   (merged with `opts.react.system_prompt` if both provided;
   persona wins, user prompt is appended as an override block).
2. Persona → `CallOptions` adjustments, applied inside a
   `chat_model_fn` wrapper (reuses
   `rustakka_langgraph_prebuilt::providers_adapter::chat_model_fn`).
3. IQ `planning_hops` → `CompileConfig.recursion_limit` when the
   user hasn't set one.
4. EQ `reflection_cadence` → optional `reflect` node inserted into the
   graph topology before compilation.

The wrappers never touch engine internals; they only call the
existing public APIs already exercised by upstream tests.

---

## 5a. Prebuilt research & harness graphs

Beyond the minimal persona wrappers of § 5, `rustakka-agent-prebuilt`
ships opinionated, end-to-end agent graphs that are ready to compile.
They are expressed purely in terms of `rustakka-langgraph` primitives
(`StateGraph`, `NodeKind`, `ChannelSpec`, tool-condition routers) so
they benefit automatically from checkpointing, streaming, store
injection, and visualization.

### 5a.1 Deep research graph — AI-Q-style (`aiq_research`)

Modeled after the
[NVIDIA AI-Q Research Agent Blueprint](https://docs.nvidia.com/aiq-blueprint/2.0.0/architecture/overview.html)
(overview, deep-researcher, shallow-researcher). Port the *shape*
of the blueprint — orchestrator state machine, intent classifier,
shallow/deep split, planner → researcher → critic loop, citation
verification middleware — into native rustakka-langgraph, with all
components swappable via personas and the IQ ladder.

#### Topology

```text
                     ┌───────────────┐
   user query ─────▶ │  clarifier    │◀── optional HITL approval
                     └──────┬────────┘
                            ▼
                     ┌───────────────┐
                     │ intent        │  (single-LLM classifier)
                     │ classifier    │
                     └───┬──────┬────┘
                "shallow"│      │"deep"
                         ▼      ▼
      ┌──────────────────┐   ┌──────────────────────────────┐
      │ shallow researcher│  │ deep researcher orchestrator │
      │ (tight tool loop) │  │                              │
      └────────┬─────────┘   │  ┌───────────┐               │
               │             │  │  planner  │               │
               │             │  └─────┬─────┘               │
               │             │        ▼                     │
               │             │  ┌───────────┐  fan-out      │
               │             │  │researcher │──┬─▶ evidence │
               │             │  └─────┬─────┘  ├─▶ comparator│
               │             │        ▼        └─▶ critic   │
               │             │  ┌───────────┐               │
               │             │  │ synthesizer│              │
               │             │  └─────┬─────┘               │
               │             └────────┼──────────────────────┘
               │                      ▼
               └──────────▶ ┌────────────────────┐
                             │ citation verifier /│
                             │ report sanitizer   │ (middleware)
                             └─────────┬──────────┘
                                       ▼
                                   final report
```

#### Crate + API

- Lives in `rustakka-agent-prebuilt::aiq_research`.
- Behind a Cargo feature `aiq-research` on the umbrella crate so
  small deployments don't pay for it.

```rust
pub struct AiqResearchOptions {
    pub persona: Option<Persona>,
    pub ladder: IqLadder,                        // drives per-subagent models
    pub allow_deep_path: bool,                   // false ⇒ always shallow
    pub hitl_clarifier: bool,                    // enable human-in-the-loop
    pub ensemble: Option<EnsembleConfig>,        // optional parallel runs
    pub post_hoc_refiner: bool,                  // polish final report
    pub citation_verifier: Arc<dyn CitationVerifier>,
    pub tools: AiqToolkit,                       // search / retriever / code …
}

pub async fn create_aiq_research_agent(
    opts: AiqResearchOptions,
) -> GraphResult<CompiledStateGraph>;
```

#### Subagent / tier mapping

The default IQ ladder assigns tiers per subagent; all assignments are
overridable via `AiqResearchOptions`.

| Subagent               | Default IQ tier | Default carryings                          |
|------------------------|-----------------|--------------------------------------------|
| `clarifier`            | `Operator`      | low temp, short max-tokens                 |
| `intent_classifier`    | `Reflex`        | temp=0.0, max_tokens=32, cache=long        |
| `shallow_researcher`   | `Analyst`       | tool-enabled tight ReAct loop              |
| `planner`              | `Strategist`    | high max-tokens, reflection on             |
| `researcher` (root)    | `Strategist`    | fan-out enabled                            |
| `evidence_gatherer`    | `Analyst`       | retrieval tools only                       |
| `comparator`           | `Analyst`       | compare/contrast prompt fragments          |
| `critic`               | `Strategist`    | self-critique prompt fragments             |
| `synthesizer`          | `Scholar`       | largest max-tokens, ensemble-aware         |
| `post_hoc_refiner`     | `Strategist`    | style-normalizing prompt fragments         |

#### State

```rust
/// Namespaced channels written into the graph state. Names are
/// stable so downstream visualizations / checkpoint inspectors can
/// rely on them.
pub mod channels {
    pub const MESSAGES: &str        = "messages";           // add_messages
    pub const INTENT: &str          = "aiq.intent";         // "shallow"|"deep"
    pub const PLAN: &str            = "aiq.plan";           // Value (JSON)
    pub const EVIDENCE: &str        = "aiq.evidence";       // append list
    pub const CRITIQUES: &str       = "aiq.critiques";      // append list
    pub const CITATIONS: &str       = "aiq.citations";      // append list
    pub const REPORT: &str          = "aiq.report";         // final string
    pub const SANITIZATION_LOG: &str = "aiq.sanitization";  // append list
}
```

`CitationVerifier` and `ReportSanitizer` are public traits with a
default implementation (`DefaultCitationVerifier`) that performs HEAD
requests (or, in `AgentEnv::Test`, uses a deterministic fixture).

#### Streaming / checkpointing

- Multi-mode streaming and `subgraphs=True` work out-of-the-box —
  the graph is just another `CompiledStateGraph`.
- A long-running deep-research run is expected to be paired with a
  `PostgresSaver` + `Durability::Async` in production, and
  `MemorySaver` in dev/test.

#### Non-goals for the port

- We do **not** replicate NVIDIA AI-Q's prompt text verbatim — only
  the topology, state, and routing semantics. Prompt content is
  persona-rendered.
- We do **not** bundle NIM endpoints or vendor-specific tools; those
  plug in via `rustakka-langgraph-providers` and the standard
  `Tool` registration path.

### 5a.2 Harness graph — Oh-My-OpenAgent-style (`omo_harness`)

Modeled after the
[Oh My OpenAgent](https://github.com/code-yeongyu/oh-my-openagent)
harness: a hierarchical, category-routed multi-agent orchestrator
(Sisyphus/Prometheus/Hephaestus/… are the canonical names in the
original). We re-express the *pattern* in rustakka-agent terms:

- One **orchestrator** persona that parses intent and delegates.
- A set of **discipline** personas routed by **category**, not by
  model name — the IQ ladder picks the actual model per tier.
- A **session-continuity** channel (`omo.boulder`) mirroring the
  original `boulder.json` so a graph resumed from a checkpoint
  continues the same boulder.
- Optional **hash-anchored edit** middleware for code-editing
  subagents (`omo.hashline`) that refuses edits whose anchor hash
  has drifted.

#### Canonical disciplines

Shipped as a default set; every discipline is a `PersonaAgent`
assignable to any `ChatModel` ladder rung.

| Discipline     | Default persona role      | Default category      | Default IQ tier |
|----------------|---------------------------|-----------------------|-----------------|
| `sisyphus`     | primary orchestrator       | `orchestration`       | `Strategist`    |
| `prometheus`   | strategic planner          | `planning`            | `Strategist`    |
| `hephaestus`   | autonomous deep worker     | `deep`                | `Scholar`       |
| `oracle`       | architecture reviewer      | `ultrabrain`          | `Strategist`    |
| `librarian`    | docs / context curator     | `documentation`       | `Analyst`       |
| `explore`      | codebase / corpus search   | `search`              | `Operator`      |
| `visio`        | UI / visual work           | `visual-engineering`  | `Analyst`       |
| `quick`        | single-file touch-ups      | `quick`               | `Reflex`        |

#### Topology

```text
                          ┌────────────┐
  user ──▶ IntentGate ───▶│  sisyphus  │◀─ returns here each hop
                          └────┬───────┘
                               │ category routing (persona-aware)
        ┌─────────┬─────────┬──┼──┬─────────┬─────────┐
        ▼         ▼         ▼  ▼  ▼         ▼         ▼
    prometheus hephaestus oracle librarian explore  visio   quick
        │         │         │    │         │         │       │
        └─────────┴─────────┴────┴─────────┴─────────┴───────┘
                               ▼
                          BoulderStore   (session continuity)
                               ▼
                          HashlineGate   (edit safety middleware)
                               ▼
                              END
```

Under the hood this is `create_persona_supervisor(...)` from § 5
with:

- a persona-aware `SupervisorRouter` that consults the
  orchestrator's intent output **and** each discipline persona's
  declared categories,
- a `BoulderStore` channel built on
  `rustakka-langgraph-store::InMemoryStore` (or `PostgresStore` in
  prod) tagging each task with a stable ID,
- an optional `HashlineGate` node inserted between discipline
  agents and any tools tagged `category=edit`.

#### Crate + API

- Lives in `rustakka-agent-prebuilt::omo_harness`.
- Behind a Cargo feature `omo-harness`.

```rust
pub struct OmoHarnessOptions {
    pub ladder: IqLadder,
    pub orchestrator: PersonaAgent,
    pub disciplines: Vec<PersonaAgent>,          // defaults available
    pub boulder_store: Option<Arc<dyn BaseStore>>,
    pub hashline: HashlineMode,                  // Off | Warn | Enforce
    pub default_set: bool,                       // install canonical defaults
}

pub async fn create_omo_harness(
    opts: OmoHarnessOptions,
) -> GraphResult<CompiledStateGraph>;

pub fn default_disciplines(ladder: &IqLadder) -> Vec<PersonaAgent>;
```

Minimal example (Rust):

```rust,ignore
let ladder = IqLadder::builder()
    .tier(IqTier::Scholar, openai_rung("gpt-4o"))
    .tier(IqTier::Strategist, openai_rung("gpt-4o"))
    .tier(IqTier::Analyst, openai_rung("gpt-4o-mini"))
    .tier(IqTier::Operator, openai_rung("gpt-4o-mini"))
    .tier(IqTier::Reflex, ollama_rung("llama3:8b"))
    .build();

let app = create_omo_harness(OmoHarnessOptions {
    ladder: ladder.clone(),
    orchestrator: PersonaAgent::sisyphus(&ladder),
    disciplines: default_disciplines(&ladder),
    boulder_store: Some(Arc::new(InMemoryStore::new())),
    hashline: HashlineMode::Enforce,
    default_set: true,
}).await?;
```

#### Non-goals for the port

- We do **not** reproduce Oh-My-OpenAgent's prompt text verbatim —
  only the topology, state, and routing semantics. Prompt content
  is persona-rendered.
- We do **not** bundle vendor-specific MCP servers; MCP tools plug
  in via `rustakka-langgraph-prebuilt::Tool`.

### 5a.3 Common agentic patterns catalog (`patterns`)

On top of ReAct, Supervisor, Swarm, AI-Q, and the OMO harness,
`rustakka-agent-prebuilt::patterns` ships a **catalog of small,
reusable agentic patterns** that any persona can pull into its
graph. Each pattern is:

- a standalone `create_*` factory returning a `CompiledStateGraph`,
- *and* a `Pattern` builder that can be **composed** as a subgraph
  into a larger graph via
  `CompiledStateGraph::as_subgraph_invoker(...)`,

so a persona might use plan-execute on the outside, reflexion inside
each execution step, and RAG inside each tool call.

Every pattern honors the standard rustakka-agent triplet:

1. **Persona** (`Option<Persona>`) → system-prompt fragments.
2. **IqLadder** → per-role model rung + carryings selection.
3. **`AgentEnv`** → mock provider + deterministic fixtures under
   `Test`.

#### 5a.3.1 Minimum pattern set

All patterns live in `rustakka-agent-prebuilt::patterns::*` behind
the umbrella feature `patterns` and individual sub-features so small
deployments pay only for what they import.

| Pattern                    | Module / feature                 | Shape                                                                 | Notable channels / knobs                                        |
|----------------------------|----------------------------------|-----------------------------------------------------------------------|-----------------------------------------------------------------|
| **Plan-and-Execute**       | `plan_execute` / `plan-execute`  | `planner → executor[*] → replanner? → END`                            | `plan`, `plan.steps`, `plan.cursor`, `plan.revisions`           |
| **Reflexion**              | `reflexion` / `reflexion`        | `act → evaluate → reflect → act`  (bounded by `max_reflections`)      | `reflexion.memory`, `reflexion.critique`, `reflexion.attempts`  |
| **Evaluator–Optimizer**    | `evaluator_optimizer` / `eval-opt` | `generate → evaluate → (accept? | optimize → generate)`              | `eval.score`, `eval.threshold`, `eval.rubric`                   |
| **Self-Consistency**       | `self_consistency` / `self-consistency` | `fan_out[N generators] → majority/scorer → aggregate`            | `sc.samples`, `sc.votes`, `sc.winner`                           |
| **Tree-of-Thoughts / LATS**| `tot` / `tree-of-thought`        | `expand → evaluate → select → expand …` with bounded MCTS-style search| `tot.frontier`, `tot.scores`, `tot.budget`                      |
| **Debate / Jury**          | `debate` / `debate`              | `proposer[*] → critic[*] → judge` (multi-round)                       | `debate.rounds`, `debate.arguments`, `debate.verdict`           |
| **Router / Mixture-of-Experts** | `router` / `router`         | `classifier → {expert_1, …, expert_n}`                                | `router.intent`, `router.selected`, `router.confidence`         |
| **RAG**                    | `rag` / `rag`                    | `retriever → (rerank?) → grounded_generator → cite_checker`           | `rag.query`, `rag.docs`, `rag.citations`                        |
| **Corrective RAG (CRAG)**  | `crag` / `crag`                  | `rag → self_grade → (regen | web_search → rag)` loop                  | `crag.grade`, `crag.mode ∈ {correct, ambiguous, incorrect}`     |
| **Adaptive RAG**           | `adaptive_rag` / `adaptive-rag`  | `router → {no_retrieve, single_retrieve, multi_hop_retrieve} → gen`   | `rag.strategy`                                                  |
| **Self-RAG**               | `self_rag` / `self-rag`          | `generate(with reflection tokens) → verify → regenerate?`             | `self_rag.reflect_tokens`, `self_rag.support`                   |
| **Human-in-the-Loop Gate** | `hitl_gate` / `hitl`             | `propose → interrupt(await_human) → resume`                           | `hitl.awaiting`, `hitl.decision`, `hitl.payload`                |
| **Memory-Augmented Agent** | `memory_agent` / `memory`        | ReAct with a **long-term-memory** subgraph (read/write/update/forget) | `memory.scope ∈ {session, user, world}`, `memory.store`         |
| **Toolformer / Codex loop**| `codex_loop` / `codex-loop`      | `plan → code → run → observe → repair` (bounded)                      | `codex.diff`, `codex.test_log`, `codex.attempts`                |
| **Guardrails / Policy**    | `guardrails` / `guardrails`      | `pre_check → agent → post_check` with refusal routes                  | `guard.preflight`, `guard.postflight`, `guard.refusal_reason`   |

#### 5a.3.2 Common `Pattern` trait

Each factory also exposes a typed builder so patterns can be nested:

```rust
pub trait Pattern {
    /// Crate-stable name, e.g. "plan_execute".
    fn name(&self) -> &'static str;

    /// Channels this pattern writes/reads. Used for state merging
    /// when composed as a subgraph.
    fn channels(&self) -> &[ChannelSpec];

    /// Produce a `NodeKind` that invokes this pattern as a subgraph.
    fn as_node(self: Arc<Self>) -> NodeKind;

    /// Build a standalone compiled graph.
    fn compile(self: Arc<Self>) -> GraphResult<CompiledStateGraph>;
}
```

Concrete examples of composition:

```rust,ignore
// "Deep persona" = plan-execute on the outside, reflexion inside
// every executor step, RAG inside every tool call.
let rag       = rag::Builder::new(retriever).compile_pattern()?;
let reflexion = reflexion::Builder::new()
    .max_reflections(3)
    .inner(executor_fn_backed_by(rag.clone()))
    .compile_pattern()?;
let app = plan_execute::Builder::new()
    .planner(planner_node(&ladder, IqTier::Strategist))
    .executor(reflexion.clone().as_node())
    .replanner(true)
    .persona(Some(persona))
    .ladder(ladder)
    .compile()
    .await?;
```

#### 5a.3.3 Persona + ladder integration

Every pattern factory accepts `Option<Persona>` and `IqLadder` and
applies them uniformly:

- Role → tier mapping is **pattern-specific**: a planner node runs
  at `Strategist`, an evaluator at `Analyst`, a majority aggregator
  at `Reflex`, etc. These mappings are overridable via a
  `RoleTierMap` passed in the builder.
- Persona `to_system_prompt()` is split by **role**
  (`persona.role_fragment("planner")`, `…("critic")`, …) so that a
  single persona can modulate every node in the pattern
  consistently.
- EQ `reflection_cadence` silently upgrades ReAct-style patterns to
  their reflexion variants when set to `AfterEachTurn`.

#### 5a.3.4 State & namespacing rules

- All pattern-written channels live under a **namespace** matching
  the pattern name: `plan.*`, `reflexion.*`, `rag.*`, … so that
  composing patterns never collides on channel keys.
- Every pattern defines stable constants
  (`pub mod channels { pub const … }`) so downstream checkpoint
  inspectors, visualizations, and tests can rely on them.
- Interrupt-capable patterns (Plan-Execute's replanner, HITL gate,
  Reflexion's reflection, Debate's judge) **must** use the upstream
  `interrupt_before` / `interrupt_after` mechanism — they do not
  implement their own pause primitive.

#### 5a.3.5 Tests & fixtures

- A shared `tests/patterns_fixtures/` directory holds deterministic
  scripted `MockChatModel` transcripts. Each pattern has:
  - a **happy-path** test (expected trajectory),
  - a **bound-exhaustion** test (max reflections / samples / rounds
    reached → graceful finalization),
  - a **composition** test nesting the pattern inside at least one
    other pattern.
- Snapshot tests (`insta`) lock the final state payload so any
  prompt- or topology-change is an explicit, reviewable diff.

#### 5a.3.6 Non-goals

- No pattern bundles a real tool implementation; tools plug in via
  `rustakka-langgraph-prebuilt::Tool`.
- We do not ship a DSL for authoring new patterns. Authors write a
  normal Rust module implementing `Pattern`. A macro
  (`#[pattern(name = "…")]`) is listed as a Phase-9 follow-up.
- Patterns are not a replacement for AI-Q or OMO — those are
  opinionated, end-to-end agents. Patterns are *ingredients*.

---

## 6. Python façade

Mirrors the `rustakka-langgraph` approach:

- `py-bindings/pyagent` — `pyo3` cdylib building
  `rustakka_agent._native`.
- `python/rustakka_agent/` — a thin Python package with:
  - `Persona`, `IqProfile`, `EqProfile`, `Trait` dataclasses whose
    `.to_native()` round-trips through the cdylib.
  - Helpers `create_react_agent(..., persona=...)`,
    `create_supervisor(..., persona=...)`,
    `create_swarm(agents=[PersonaAgent(...), ...])` that shadow the
    upstream `langgraph.prebuilt` functions and delegate to them
    after applying the persona.
- `python/tests/` — pytest suite with parity tests against the
  upstream Python behavior (persona=None must match upstream exactly)
  and with deterministic `MockChatModel` to avoid network flakiness.

---

## 7. Environment awareness

Every crate reads `RUSTAKKA_AGENT_ENV`:

| Env    | Default provider       | Logging       | Safety rails       | Persona validation |
|--------|------------------------|---------------|--------------------|--------------------|
| `dev`  | real provider if keys  | `debug`       | relaxed            | warn on conflicts  |
| `test` | `MockChatModel` always | `info`, JSON  | strictest          | fail on conflicts  |
| `prod` | real provider required | `info`        | strict             | fail on conflicts  |

This is implemented in a small `rustakka-agent-traits::env` module so
every crate can call `AgentEnv::current()` without duplication.

---

## 8. Phase-by-phase execution plan

The plan is deliberately matched to the 0–9 cadence of
`rustakka-langgraph` so we can share CI scripts and tracking.

### Phase 0 — Workspace scaffold
- Cargo workspace, `rust-toolchain.toml`, `rustfmt.toml`.
- `crates/rustakka-agent-traits` with `Trait`, `Score`, `Dimension`,
  `TraitSet`, `AgentEnv`.
- CI workflow cloned from `rustakka-langgraph` (`cargo fmt --check`,
  `cargo clippy -- -D warnings`, `cargo test --workspace`).
- `docs/plan.md`, `docs/TODO.md`.

### Phase 1 — IQ / EQ profiles
- `rustakka-agent-iq` and `rustakka-agent-eq` crates.
- Builder APIs, serde round-trips, `to_prompt_fragment`.
- Unit tests for clamping, merging, prompt assembly determinism.

### Phase 1b — IQ tiers & model ladder
- `IqTier` enum + `IqProfile::tier()` composite-score inference
  with `pinned_tier` override.
- `IqCarryings` struct + fold order (ladder-default → tier → rung →
  persona → caller).
- `ModelRung`, `TierLadder`, `IqLadder`, `IqLadderBuilder`.
- YAML/TOML loader + authoritative schema in `docs/iq-ladders.md`.
- `AgentEnv::Test` forces `MockChatModel` regardless of rung.
- Unit tests: tier bucketing edge cases, carryings fold order,
  predicate-based rung selection, env-forced mock.

### Phase 2 — Persona core
- `rustakka-agent-persona` crate with `Persona`, `Identity`,
  `CommunicationStyle`, `MemoryPrefs`, `SafetyRails`.
- Loaders for TOML / YAML / JSON.
- `validate()` with targeted error types.
- `to_system_prompt()` with snapshot tests
  (`insta`) for stability.

### Phase 3 — Prebuilt integration
- `rustakka-agent-prebuilt` crate.
- `create_persona_react_agent` + tests using a `MockChatModel`.
- Verify `persona=None` is behaviorally identical to calling
  upstream `create_react_agent` directly (parity test).

### Phase 4 — Supervisor & swarm
- `create_persona_supervisor` and `create_persona_swarm`.
- Router helpers (`persona_based_router`) that consult persona
  values when multiple agents can service a request.
- Multi-agent integration tests.

### Phase 5 — Reflection & tool biasing
- Optional `reflect` node injected from `EqProfile.reflection_cadence`.
- `tools_condition` biasing from `IqProfile.tool_eagerness`.
- End-to-end test: a "cautious" persona skips unsafe tools.

### Phase 5a — AI-Q-style deep-research graph
- `rustakka-agent-prebuilt::aiq_research` module (Cargo feature
  `aiq-research`).
- Clarifier (HITL) → intent classifier → shallow / deep split.
- Deep path: planner → researcher (fan-out: evidence gatherer,
  comparator, critic) → synthesizer → post-hoc refiner.
- `CitationVerifier` + `ReportSanitizer` traits with default impls
  and a deterministic fixture-backed impl for `AgentEnv::Test`.
- Stable channel namespace (`aiq.intent`, `aiq.plan`, `aiq.evidence`,
  `aiq.critiques`, `aiq.citations`, `aiq.report`,
  `aiq.sanitization`).
- Per-subagent default IQ-tier mapping; overridable via
  `AiqResearchOptions`.
- Integration tests with a mock provider: shallow path returns a
  single turn; deep path runs ≥ 2 researcher fan-outs and produces
  a sanitized report.
- `examples/rust_aiq_research` + `docs/aiq-research.md`.

### Phase 5b — Oh-My-OpenAgent-style harness graph
- `rustakka-agent-prebuilt::omo_harness` module (Cargo feature
  `omo-harness`).
- Canonical discipline personas (sisyphus, prometheus, hephaestus,
  oracle, librarian, explore, visio, quick) as
  `PersonaAgent`s assembled under `create_persona_supervisor`.
- Persona-aware category router consulting orchestrator intent
  plus each discipline's declared categories.
- `BoulderStore` channel (`omo.boulder`) on top of
  `rustakka-langgraph-store` for session continuity across
  checkpoints.
- `HashlineGate` edit-safety middleware (`Off | Warn | Enforce`),
  wired in automatically for any tool tagged `category=edit`.
- Integration tests: task routes to the right discipline based on
  category; resumed run picks up the same boulder; enforced
  hashline rejects stale edits.
- `examples/rust_omo_harness` + `docs/omo-harness.md`.

### Phase 5c — Common agentic patterns catalog
- `rustakka-agent-prebuilt::patterns::*` behind an umbrella
  `patterns` feature plus per-pattern sub-features.
- Implement the minimum set:
  `plan_execute`, `reflexion`, `evaluator_optimizer`,
  `self_consistency`, `tot` (Tree-of-Thoughts / LATS), `debate`,
  `router`, `rag`, `crag`, `adaptive_rag`, `self_rag`,
  `hitl_gate`, `memory_agent`, `codex_loop`, `guardrails`.
- Shared `Pattern` trait (`name`, `channels`, `as_node`, `compile`)
  so patterns compose as subgraphs.
- Stable channel namespaces per pattern (`plan.*`, `reflexion.*`,
  `rag.*`, `crag.*`, `debate.*`, `hitl.*`, `memory.*`, …).
- Role → tier mapping + `RoleTierMap` override so each internal
  node picks the right ladder rung.
- `EqProfile.reflection_cadence == AfterEachTurn` silently upgrades
  ReAct-style patterns to their reflexion variants.
- Tests per pattern: happy-path, bound-exhaustion, composition
  (pattern nested inside another pattern).
- `docs/patterns.md` + four examples (plan-execute, reflexion,
  RAG suite, debate).

### Phase 6 — Umbrella crate + profiler
- `rustakka-agent` facade re-exports with `persona`, `prebuilt`,
  `providers` feature flags.
- `rustakka-agent-profiler` with scenarios:
  `persona-compile`, `prompt-render`, `react-turn`.

### Phase 7 — Python bindings
- `pyagent` cdylib + `python/rustakka_agent` package.
- pytest parity suite.
- `maturin develop` smoke test in CI.

### Phase 8 — Docs & examples
- `examples/rust_persona_react`, `examples/rust_supervisor_team`,
  `examples/rust_aiq_research`, `examples/rust_omo_harness`,
  `examples/rust_pattern_plan_execute`,
  `examples/rust_pattern_reflexion`,
  `examples/rust_pattern_rag_suite`,
  `examples/rust_pattern_debate`.
- `docs/persona-schema.md` (authoritative JSON schema for personas).
- `docs/iq-ladders.md` (authoritative schema for ladders + carryings).
- `docs/aiq-research.md` / `docs/omo-harness.md` / `docs/patterns.md`
  — topology, routing, middleware, composition references.
- `docs/integration.md` (how to migrate existing
  `create_react_agent` callers).

### Phase 9 — Hardening
- Fuzzing `Persona::from_*` loaders (`cargo fuzz`).
- Golden-file tests for `to_system_prompt` across locales.
- Benchmarks committed as baselines under `docs/benchmarks/`.
- Safety-rails red-team suite (prompt-injection resistance checks
  using a deterministic mock provider).

---

## 9. Testing strategy

- **Unit tests** live alongside source (`#[cfg(test)] mod tests`).
- **Integration tests** (`crates/*/tests/`) run compiled graphs
  against a `MockChatModel` from `rustakka-langgraph-providers::mock`.
- **Snapshot tests** (`insta`) lock `to_system_prompt` output, making
  any prompt-behavior change an explicit, reviewable diff.
- **Parity tests** assert: *with* persona and *without* persona,
  given identical inputs and a deterministic mock model, the engine
  produces byte-identical state transcripts when persona effects are
  disabled.
- **Python tests** (`pytest -v`) mirror the Rust integration tests
  through the PyO3 bridge.

Target coverage at v1.0: ≥ 90 % line coverage on the `traits`,
`iq`, `eq`, `persona` crates; ≥ 80 % on `prebuilt`.

---

## 10. Risks & mitigations

| Risk | Mitigation |
|------|------------|
| Prompt non-determinism breaks snapshot tests | Deterministic ordering in `to_system_prompt` via `BTreeMap`; locale-aware casing gated behind an explicit feature flag. |
| Upstream `rustakka-langgraph` API churn | Pin to a minor version; CI matrix tests against `main` and the pinned release. |
| Persona config drift (TOML ↔ JSON ↔ YAML) | Single canonical serde schema; the YAML/TOML loaders are thin wrappers over the JSON schema. |
| Persona features bleed into engine semantics | Firewall: only `rustakka-agent-prebuilt` depends on `rustakka-langgraph-prebuilt`; everything else is pure data + logic. |
| Python/Rust drift | Shared fixtures; pytest runs the same persona JSON files Rust uses. |

---

## 11. Out-of-scope follow-ups

Ideas captured but explicitly deferred:

- Persona *learning* (updating traits from conversation outcomes).
- Multi-modal persona attributes (voice, avatar).
- Persona marketplace / remote persona registry.
- Reinforcement-style reward models keyed on EQ metrics.
