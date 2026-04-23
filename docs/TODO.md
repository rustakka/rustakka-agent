# rustakka-agent — Implementation progress

Phases mirror the plan at [`docs/plan.md`](plan.md). Update the
checkboxes as PRs land.

- [ ] **Phase 0** — Workspace scaffold + `rustakka-agent-traits`
      (`Trait`, `Score`, `Dimension`, `TraitSet`, `AgentEnv`) + CI
- [ ] **Phase 1** — `rustakka-agent-iq` + `rustakka-agent-eq`
      profiles with builders, serde, and `to_prompt_fragment`
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
- [ ] **Phase 6** — Umbrella `rustakka-agent` crate (feature gates)
      and `rustakka-agent-profiler` scenarios
- [ ] **Phase 7** — `pyagent` cdylib + `python/rustakka_agent`
      package + pytest parity suite
- [ ] **Phase 8** — Examples (`rust_persona_react`,
      `rust_supervisor_team`) + `docs/persona-schema.md` +
      `docs/integration.md`
- [ ] **Phase 9** — Hardening: fuzzing loaders, golden-file prompt
      tests, committed benchmarks, safety red-team suite

## Current status

The repository currently contains **only the plan and a scaffold
workspace**; no phases have started. See [`docs/plan.md`](plan.md)
§ 8 for entry criteria.
