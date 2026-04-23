# AI-Q-style deep-research graph

Source: [`rustakka-agent-prebuilt::aiq_research`](../crates/rustakka-agent-prebuilt/src/aiq_research/).
Cargo feature: `aiq-research` (enabled by default on the umbrella
crate).

Modeled after the
[NVIDIA AI-Q Research Agent Blueprint](https://docs.nvidia.com/aiq-blueprint/2.0.0/architecture/overview.html) —
we port the *topology* and *state* of the blueprint, not its prompts
or vendor-specific NIM bindings.

## Topology

```
                   ┌───────────────┐
 user query ─────▶ │  clarifier    │◀── optional HITL (interrupt_before)
                   └──────┬────────┘
                          ▼
                   ┌───────────────┐
                   │ intent        │  (single-LLM classifier)
                   │ classifier    │
                   └───┬──────┬────┘
              "shallow"│      │"deep"
                       ▼      ▼
    ┌──────────────────┐   ┌──────────────────────────────┐
    │ shallow researcher│  │   planner                     │
    │ (tight tool loop) │  │   → researcher                │
    └────────┬─────────┘   │     ├─▶ evidence_gatherer     │
             │             │     ├─▶ comparator            │
             │             │     └─▶ critic                │
             │             │   → synthesizer               │
             │             └───────────────┬───────────────┘
             ▼                             ▼
             └─────▶ citation_verifier ◀───┘
                          ▼
                 post_hoc_refiner? → END
```

## Channels

All channels live under the `aiq.*` namespace so they never collide
with pattern or harness channels.

| Channel                  | Kind         | Written by              |
|--------------------------|--------------|-------------------------|
| `messages`               | `Messages`   | Every node              |
| `aiq.intent`             | `LastValue`  | `intent_classifier`     |
| `aiq.plan`               | `LastValue`  | `planner`               |
| `aiq.evidence`           | `AppendList` | `evidence_gatherer`     |
| `aiq.critiques`          | `AppendList` | `critic`                |
| `aiq.citations`          | `AppendList` | `researcher`, `shallow_researcher` |
| `aiq.report`             | `LastValue`  | `synthesizer`, `post_hoc_refiner` |
| `aiq.sanitization`       | `AppendList` | `ReportSanitizer`       |
| `aiq.ensemble.runs.<N>`  | `LastValue`  | set when `EnsembleConfig` is present |

## Default per-subagent IQ tiers

See `default_subagent_tiers()` in
[`aiq_research/mod.rs`](../crates/rustakka-agent-prebuilt/src/aiq_research/mod.rs).
Overridable via `AiqResearchOptions` + a custom `RoleTierMap`.

| Subagent             | Default tier |
|----------------------|--------------|
| `clarifier`          | `Operator`   |
| `intent_classifier`  | `Reflex`     |
| `shallow_researcher` | `Analyst`    |
| `planner`            | `Strategist` |
| `researcher`         | `Strategist` |
| `evidence_gatherer`  | `Analyst`    |
| `comparator`         | `Analyst`    |
| `critic`             | `Strategist` |
| `synthesizer`        | `Scholar`    |
| `post_hoc_refiner`   | `Strategist` |

## `AiqResearchOptions`

```rust
pub struct AiqResearchOptions {
    pub persona: Option<Persona>,
    pub ladder: IqLadder,
    pub allow_deep_path: bool,           // false ⇒ shallow-only
    pub hitl_clarifier: bool,            // insert interrupt_before("clarifier")
    pub ensemble: Option<EnsembleConfig>,
    pub post_hoc_refiner: bool,
    pub citation_verifier: Arc<dyn CitationVerifier>,
    pub sanitizer: Arc<dyn ReportSanitizer>,
    pub tools: AiqToolkit,
}
```

Default `CitationVerifier` accepts every citation; the
`FixtureCitationVerifier` (used when `AgentEnv::Test`) accepts only
non-empty citations, so test snapshots are stable across machines.

## Streaming / checkpointing

The graph is a plain `CompiledGraph`, so all upstream capabilities
apply:

- Multi-mode streaming (messages / tokens / graph events) works out
  of the box.
- A deep run in prod is typically paired with `PostgresSaver` +
  `Durability::Async`; dev/test use `MemorySaver`.

## Non-goals

- We do not replicate the blueprint's prompt text. Prompts are
  persona-rendered.
- We do not bundle NIM endpoints or vendor-specific tools. Tools plug
  in via the standard `Tool` registration path.
