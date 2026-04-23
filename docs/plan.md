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
                               # + aiq_research graph
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
docs/
  plan.md                      # (this file)
  TODO.md
  persona-schema.md
  iq-ladders.md                # YAML/TOML schema for IqLadder / carryings
  aiq-research.md              # AI-Q graph topology, state, middleware
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

## 5a. Prebuilt research graphs

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
  `examples/rust_aiq_research`.
- `docs/persona-schema.md` (authoritative JSON schema for personas).
- `docs/iq-ladders.md` (authoritative schema for ladders + carryings).
- `docs/aiq-research.md` — topology, routing, middleware reference.
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
