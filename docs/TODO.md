# rustakka-agent — Implementation progress

Phases mirror the plan at [`docs/plan.md`](plan.md). Update the
checkboxes as PRs land.

- [ ] **Phase 0** — Workspace scaffold + `rustakka-agent-traits`
      (`Trait`, `Score`, `Dimension`, `TraitSet`, `AgentEnv`) + CI
- [ ] **Phase 1** — `rustakka-agent-iq` + `rustakka-agent-eq`
      profiles with builders, serde, and `to_prompt_fragment`
- [ ] **Phase 1b** — IQ tiers + model ladder:
      - [ ] `IqTier` enum + composite-score inference +
            `pinned_tier` override
      - [ ] `IqCarryings` struct with deterministic fold order
            (ladder → tier → rung → persona → caller)
      - [ ] `ModelRung` / `TierLadder` / `IqLadder` /
            `IqLadderBuilder`
      - [ ] YAML/TOML loader + `docs/iq-ladders.md` schema
      - [ ] `AgentEnv::Test` forces `MockChatModel` regardless of
            rung
      - [ ] Unit tests: bucketing, fold order, predicate selection,
            env-forced mock
- [ ] **Phase 2** — `rustakka-agent-persona` core: `Persona`,
      `Identity`, `CommunicationStyle`, `MemoryPrefs`, `SafetyRails`,
      TOML/YAML/JSON loaders, snapshot-tested `to_system_prompt`
- [ ] **Phase 3** — `rustakka-agent-prebuilt::create_persona_react_agent`
      (parity + persona-enabled paths)
- [ ] **Phase 4** — `create_persona_supervisor` +
      `create_persona_swarm` + persona-aware routers
- [ ] **Phase 5** — Reflection node injection from
      `EqProfile.reflection_cadence`; `tools_condition` biasing from
      `IqProfile.tool_eagerness`
- [ ] **Phase 5a** — AI-Q-style deep-research graph
      (`rustakka-agent-prebuilt::aiq_research`, feature
      `aiq-research`):
      - [ ] Clarifier (HITL) → intent classifier → shallow/deep
            split
      - [ ] Planner → researcher (fan-out: evidence, comparator,
            critic) → synthesizer → post-hoc refiner
      - [ ] `CitationVerifier` + `ReportSanitizer` traits + default
            impls + fixture-backed impl under `AgentEnv::Test`
      - [ ] Stable channels: `aiq.intent`, `aiq.plan`,
            `aiq.evidence`, `aiq.critiques`, `aiq.citations`,
            `aiq.report`, `aiq.sanitization`
      - [ ] Per-subagent default IQ-tier mapping
      - [ ] Integration tests (mock provider, shallow + deep paths)
      - [ ] `examples/rust_aiq_research` + `docs/aiq-research.md`
- [ ] **Phase 5b** — Oh-My-OpenAgent-style harness graph
      (`rustakka-agent-prebuilt::omo_harness`, feature
      `omo-harness`):
      - [ ] Canonical discipline personas (sisyphus, prometheus,
            hephaestus, oracle, librarian, explore, visio, quick)
      - [ ] Persona-aware category router consulting orchestrator
            intent + discipline categories
      - [ ] `BoulderStore` session-continuity channel on top of
            `rustakka-langgraph-store`
      - [ ] `HashlineGate` (Off | Warn | Enforce) wired in for
            tools tagged `category=edit`
      - [ ] Integration tests: routing by category, boulder resume,
            stale-edit rejection
      - [ ] `examples/rust_omo_harness` + `docs/omo-harness.md`
- [ ] **Phase 6** — Umbrella `rustakka-agent` crate (feature gates
      including `aiq-research` and `omo-harness`) and
      `rustakka-agent-profiler` scenarios
- [ ] **Phase 7** — `pyagent` cdylib + `python/rustakka_agent`
      package + pytest parity suite
- [ ] **Phase 8** — Examples (`rust_persona_react`,
      `rust_supervisor_team`, `rust_aiq_research`,
      `rust_omo_harness`) + `docs/persona-schema.md` +
      `docs/iq-ladders.md` + `docs/aiq-research.md` +
      `docs/omo-harness.md` + `docs/integration.md`
- [ ] **Phase 9** — Hardening: fuzzing loaders (persona + ladder),
      golden-file prompt tests, committed benchmarks, safety
      red-team suite

## Current status

The repository currently contains **only the plan and a scaffold
workspace**; no phases have started. See [`docs/plan.md`](plan.md)
§ 8 for entry criteria.
