# rustakka-agent

`rustakka-agent` is an extension of
[`rustakka-langgraph`](https://github.com/rustakka/rustakka-langgraph)
that layers a **first-class agent-characteristics model** on top of the
LangGraph-compatible Pregel engine.

Where `rustakka-langgraph` gives you *the mechanics* of an agentic
graph — nodes, channels, checkpointers, streaming, ReAct/Supervisor/Swarm
prebuilts — `rustakka-agent` gives you *the personality* of the agents
that run inside those graphs:

- **IQ** — cognitive profile (reasoning depth, planning hops, tool-use
  aggressiveness, verbosity, model/temperature selection).
- **EQ** — emotional profile (empathy, tone, mood, reflection cadence,
  conflict de-escalation, affective mirroring).
- **Persona** — an optional, strongly-typed bundle of *non-physical*
  characteristics: identity, role, values, goals, communication style,
  taboos, knowledge domains, memory preferences, and safety rails.

Nothing in this library is hard-coupled to a specific LLM; personas
compile down to:

1. a system-prompt fragment injected into any `rustakka-langgraph`
   agent (ReAct, Supervisor, Swarm),
2. a set of `CallOptions` tweaks (temperature, top-p, max-tokens,
   stop-sequences), and
3. a set of *graph knobs* (recursion limit, reflection-node inclusion,
   tool allow-list) applied at compile time.

See [`docs/plan.md`](docs/plan.md) for the full design and
[`docs/TODO.md`](docs/TODO.md) for the phase checklist.

> Status: **planning**. The crates under `crates/` are stubs whose
> public APIs track the plan and compile cleanly; real behavior is
> filled in phase-by-phase.
