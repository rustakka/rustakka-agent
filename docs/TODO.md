# rustakka-agent — Implementation progress

Phases mirror the plan at [`docs/plan.md`](plan.md).

- [x] **Phase 0** — Workspace scaffold + `rustakka-agent-traits`
      (`Trait`, `Score`, `Dimension`, `TraitSet`, `AgentEnv`)
- [x] **Phase 1** — `rustakka-agent-iq` + `rustakka-agent-eq`
      profiles with builders, serde, and `to_prompt_fragment`
- [x] **Phase 1b** — IQ tiers + model ladder:
      - [x] `IqTier` enum + composite-score inference +
            `pinned_tier` override
      - [x] `IqCarryings` struct with deterministic fold order
            (ladder → tier → rung → persona → caller)
      - [x] `ModelRung` / `TierLadder` / `IqLadder` /
            `IqLadderBuilder`
      - [x] JSON loader (`IqLadderSpec::from_json`) + binder
      - [x] `AgentEnv::Test` forces `MockChatModel` regardless of
            rung
      - [x] Unit tests: bucketing, fold order, predicate selection,
            env-forced mock
- [x] **Phase 2** — `rustakka-agent-persona` core: `Persona`,
      `Identity`, `CommunicationStyle`, `MemoryPrefs`, `SafetyRails`,
      TOML/YAML/JSON loaders, deterministic `to_system_prompt`
- [x] **Phase 3** — `rustakka-agent-prebuilt::create_persona_react_agent`
      (parity + persona-enabled paths) directly on top of
      `rustakka_langgraph_prebuilt::react_agent::create_react_agent`
      (the historical mock seam has been retired)
- [x] **Phase 4** — `create_persona_supervisor` +
      `create_persona_swarm` + `persona_based_router`
- [x] **Phase 5** — Reflection node injection from
      `EqProfile.reflection_cadence`; tool biasing from
      `IqProfile.tool_eagerness`
- [x] **Phase 5a** — AI-Q-style deep-research graph
      (`rustakka-agent-prebuilt::aiq_research`, feature
      `aiq-research`):
      - [x] Clarifier (HITL) → intent classifier → shallow/deep
            split
      - [x] Planner → researcher (fan-out: evidence, comparator,
            critic) → synthesizer → post-hoc refiner
      - [x] `CitationVerifier` + `ReportSanitizer` traits + default
            + fixture impls
      - [x] Stable channels: `aiq.intent`, `aiq.plan`,
            `aiq.evidence`, `aiq.critiques`, `aiq.citations`,
            `aiq.report`, `aiq.sanitization`
      - [x] Per-subagent default IQ-tier mapping
      - [x] `examples/rust_aiq_research` + `docs/aiq-research.md`
- [x] **Phase 5b** — Oh-My-OpenAgent-style harness graph
      (`rustakka-agent-prebuilt::omo_harness`, feature
      `omo-harness`):
      - [x] Canonical discipline personas (sisyphus, prometheus,
            hephaestus, oracle, librarian, explore, visio, quick)
      - [x] Persona-aware category router on top of
            `create_persona_supervisor`
      - [x] `BaseStore` + `InMemoryStore` session-continuity
      - [x] `HashlineMode` (Off | Warn | Enforce) channel annotation
      - [x] Integration tests: discipline wiring, boulder store
            round-trip
      - [x] `examples/rust_omo_harness` + `docs/omo-harness.md`
- [x] **Phase 5c** — Common agentic patterns catalog
      (`rustakka-agent-prebuilt::patterns::*`, umbrella feature
      `patterns` + per-pattern sub-features):
      - [x] Shared `Pattern` trait + `RoleTierMap`
      - [x] `plan_execute`, `reflexion`, `evaluator_optimizer`,
            `self_consistency`, `tot`, `debate`, `router`, `rag`,
            `crag`, `adaptive_rag`, `self_rag`, `hitl_gate`,
            `memory_agent`, `codex_loop`, `guardrails` (15 total)
      - [x] Per-pattern tests + cross-pattern composition test
      - [x] `docs/patterns.md` + four examples
            (`rust_pattern_plan_execute`,
            `rust_pattern_reflexion`, `rust_pattern_rag_suite`,
            `rust_pattern_debate`)
- [x] **Phase 6** — Umbrella `rustakka-agent` crate with feature
      gates (`persona`, `prebuilt`, `patterns`, `aiq-research`,
      `omo-harness`) + `rustakka-agent-profiler` (bench scenarios:
      `persona_compile`, `prompt_render`, `react_turn`)
- [ ] **Phase 7** — `pyagent` cdylib + `python/rustakka_agent`
      package + pytest parity suite *(deferred; not required for
      the Rust-first MVP)*
- [x] **Phase 8** — Docs + examples:
      - [x] `docs/persona-schema.md`
      - [x] `docs/iq-ladders.md`
      - [x] `docs/patterns.md`
      - [x] `docs/aiq-research.md`
      - [x] `docs/omo-harness.md`
      - [x] `docs/integration.md`
      - [x] `examples/rust_persona_react`,
            `examples/rust_supervisor_team`,
            `examples/rust_aiq_research`,
            `examples/rust_omo_harness`,
            `examples/rust_pattern_plan_execute`,
            `examples/rust_pattern_reflexion`,
            `examples/rust_pattern_rag_suite`,
            `examples/rust_pattern_debate`
- [x] **Phase 9** — Hardening:
      - [x] Fuzz-lite loader tests
            (`rustakka-agent-persona::tests::fuzz_loaders`,
            `rustakka-agent-iq::tests::fuzz_ladder`) — byte-sweep
            seeds guarantee loaders return `Err`, never panic
      - [x] Golden-file prompt tests
            (`rustakka-agent-persona::tests::golden_prompts`) with
            `RUSTAKKA_UPDATE_GOLDENS=1` escape hatch
      - [x] Safety red-team suite
            (`rustakka-agent-persona::tests::safety_redteam`) —
            deny-all / taboo conflicts, deterministic safety-rail
            ordering
      - [x] Committed micro-bench scenarios in
            `rustakka-agent-profiler`

## Current status

All nine Rust-first phases are complete **and** the whole workspace
now sits directly on `rustakka-langgraph` (no trait-based mock
seam):

- `rustakka-agent-iq` owns `CallOptionsLike` / `ChatModel` traits,
  with blanket `impl`s against the upstream provider crate behind
  an opt-in `langgraph` feature (orphan-rule safe).
- `rustakka-agent-prebuilt` depends on
  `rustakka-langgraph-{core,providers,prebuilt,store}` as workspace
  deps (via local sibling checkout) and delegates to
  `create_react_agent`, `create_supervisor`, and `create_swarm`.
- Every persona-aware builder returns an `AgentGraph` that bundles
  the declarative `Blueprint` with the real upstream
  `CompiledStateGraph`.
- `cargo check --workspace --all-targets`, `cargo test --workspace
  --all-targets` (89+ tests), and `cargo clippy --workspace
  --all-targets -- -D warnings` all pass.

Phase 7 (Python bindings) remains deferred until we decide on the
py-side FFI story for the upstream crates.
