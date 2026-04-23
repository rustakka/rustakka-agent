# Agentic patterns catalog

Patterns are small, composable ingredients that live in
[`rustakka-agent-prebuilt::patterns`](../crates/rustakka-agent-prebuilt/src/patterns/).
Each pattern is both:

1. a standalone `Builder::compile()` that returns a
   `CompiledGraph`, and
2. a `Pattern` trait implementation (shared `name`, `channels`,
   `compile`) so patterns can be nested as subgraphs inside each
   other.

Every pattern honors the standard `rustakka-agent` triplet:

1. **Persona** — `Option<Persona>` → system-prompt fragments.
2. **IqLadder** — role → tier mapping via `RoleTierMap`, then tier →
   rung selection from the ladder.
3. **AgentEnv** — `AgentEnv::Test` forces `MockChatModel` and
   deterministic fixtures.

## Catalog

| Pattern                    | Module / feature                 | Shape                                                                 | Namespaced channels                                              |
|----------------------------|----------------------------------|-----------------------------------------------------------------------|------------------------------------------------------------------|
| Plan-and-Execute           | `plan_execute` / `plan-execute`  | `planner → executor[*] → replanner? → END`                            | `plan`, `plan.steps`, `plan.cursor`, `plan.revisions`            |
| Reflexion                  | `reflexion` / `reflexion`        | `act → evaluate → reflect → act` bounded by `max_reflections`         | `reflexion.memory`, `reflexion.critique`, `reflexion.attempts`   |
| Evaluator–Optimizer        | `eval_opt` / `eval-opt`          | `generate → evaluate → (accept \| optimize → generate)`               | `eval.score`, `eval.threshold`, `eval.rubric`                    |
| Self-Consistency           | `self_consistency` / `self-consistency` | `fan_out[N] → majority/scorer → aggregate`                      | `sc.samples`, `sc.votes`, `sc.winner`                            |
| Tree-of-Thoughts / LATS    | `tot` / `tree-of-thought`        | `expand → evaluate → select → expand …` MCTS-style                    | `tot.frontier`, `tot.scores`, `tot.budget`                       |
| Debate / Jury              | `debate` / `debate`              | `proposer[*] → critic[*] → judge`, multi-round                        | `debate.rounds`, `debate.arguments`, `debate.verdict`            |
| Router / MoE               | `router` / `router`              | `classifier → {expert_i}`                                             | `router.intent`, `router.selected`, `router.confidence`          |
| RAG                        | `rag` / `rag`                    | `retriever → rerank? → grounded_generator → cite_checker`             | `rag.query`, `rag.docs`, `rag.citations`                         |
| Corrective RAG (CRAG)      | `crag` / `crag`                  | `rag → self_grade → (regen \| web_search → rag)` loop                 | `crag.grade`, `crag.mode`                                        |
| Adaptive RAG               | `adaptive_rag` / `adaptive-rag`  | `router → {no_retrieve, single_retrieve, multi_hop} → gen`            | `rag.strategy`                                                   |
| Self-RAG                   | `self_rag` / `self-rag`          | `generate → verify → regenerate?`                                     | `self_rag.reflect_tokens`, `self_rag.support`                    |
| Human-in-the-Loop Gate     | `hitl_gate` / `hitl`             | `propose → interrupt(await_human) → resume`                           | `hitl.awaiting`, `hitl.decision`, `hitl.payload`                 |
| Memory-Augmented Agent     | `memory_agent` / `memory`        | ReAct with `memory_read`/`memory_write` subgraph                      | `memory.scope`, `memory.store`                                   |
| Toolformer / Codex loop    | `codex_loop` / `codex-loop`      | `plan → code → run → observe → repair` bounded                        | `codex.diff`, `codex.test_log`, `codex.attempts`                 |
| Guardrails / Policy        | `guardrails` / `guardrails`      | `pre_check → agent → post_check` with refusal routes                  | `guard.preflight`, `guard.postflight`, `guard.refusal_reason`    |

## Shared `Pattern` trait

```rust
pub trait Pattern {
    fn name(&self) -> &'static str;
    fn channels(&self) -> Vec<ChannelSpec>;
    fn compile(&self) -> GraphResult<CompiledGraph>;
}
```

Channel spec `name`s are *namespaced by the pattern* (`plan.*`,
`reflexion.*`, `rag.*`, …) so composing patterns never collides on
channel keys. This invariant is exercised in
[`tests/pattern_composition.rs`](../crates/rustakka-agent-prebuilt/tests/pattern_composition.rs).

## Composition example

```rust,ignore
use rustakka_agent_prebuilt::patterns::{plan_execute, reflexion, rag, Pattern};

// Outer pattern: plan-execute.
let outer = plan_execute::Builder::new()
    .persona(persona.clone())
    .replanner(true);

// Inner pattern: reflexion inside every executor step.
let inner = reflexion::Builder::new().max_reflections(3);

// Innermost: RAG inside every tool call.
let rag = rag::Builder::new().rerank(true).cite_check(true);

let outer_graph = outer.compile()?;
let inner_graph = inner.compile()?;
let rag_graph   = rag.compile()?;
```

## Persona + ladder integration

- Every pattern factory reads `persona.to_system_prompt()` once per
  `compile()` and writes the rendered text to the graph's
  `system_prompt`.
- Role → tier mapping comes from `RoleTierMap`. Defaults ship with
  each pattern (see each module's `Builder::new`). Callers override
  with `.roles(RoleTierMap::default().with("planner", IqTier::Scholar))`.
- `EqProfile.reflection_cadence == AfterEachTurn` is honored by the
  ReAct seam (`rustakka-agent-prebuilt::inject_reflection`); patterns
  built on ReAct therefore pick up reflection automatically.

## Tests & fixtures

Each pattern module contains at minimum:

- a **happy-path** test (topology is wired as documented),
- a **bound-exhaustion** test (`max_reflections` / `max_rounds` /
  `max_attempts` caps the recursion budget).

A shared composition test
(`tests/pattern_composition.rs`) asserts:

- channel names are disjoint across representative pairs (plan-execute
  + reflexion; RAG + debate), and
- every pattern's `compile()` produces a graph with `start` / `end`
  nodes.
