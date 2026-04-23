# IQ tiers & model ladders

This document is the authoritative schema for the IQ-tier / model-
ladder machinery implemented in
[`rustakka-agent-iq::ladder`](../crates/rustakka-agent-iq/src/ladder.rs).

## IQ tiers

| Tier         | Composite score | Intended behavior                                               |
|--------------|-----------------|-----------------------------------------------------------------|
| `Reflex`     | 0.00 – 0.20     | Tiny, reactive agents (FAQ, single-turn classifier).            |
| `Operator`   | 0.20 – 0.40     | Bounded tool loops, single-hop research.                        |
| `Analyst`    | 0.40 – 0.60     | General assistant, 2–3 hop planning.                            |
| `Strategist` | 0.60 – 0.80     | Multi-step planning, self-critique, tool teams.                 |
| `Scholar`    | 0.80 – 1.00     | Deep research, long-horizon, ensemble reasoning.                |

The composite score is computed as:

```
0.50 * reasoning_depth
  + 0.30 * normalize(planning_hops)   // soft cap at 10 hops
  + 0.20 * tool_eagerness
```

A persona can override the inferred tier with `iq.pinned_tier`.

## Carryings fold order

`IqCarryings` are folded deterministically, later writes win:

```
ladder.default_carryings
    → tier.tier_default_carryings
    → rung.carryings
    → persona (iq.temperature, …)
    → caller overrides
```

## External ladder format (JSON / YAML / TOML)

The external representation avoids embedding live `Arc<dyn
ChatModel>` instances. Callers bind rung *names* to concrete models via
`IqLadderSpec::bind(resolver)`.

```yaml
default_carryings:
  temperature: 0.2
  max_output_tokens: 1024

default_rung:
  name: gpt-4o-mini
  carryings:
    temperature: 0.0

tiers:
  Reflex:
    tier_default_carryings:
      temperature: 0.0
    rungs:
      - name: gpt-4o-mini
        carryings: { max_output_tokens: 256 }
  Scholar:
    rungs:
      - name: nemotron-ultra-253b
        carryings: { temperature: 0.5, max_output_tokens: 8192 }
      - name: gpt-4o
```

### Bind

```rust
use std::sync::Arc;
use rustakka_agent_iq::ladder::{ChatModel, IqLadderSpec};

fn resolve(name: &str) -> Option<Arc<dyn ChatModel>> {
    match name {
        "gpt-4o"       => Some(Arc::new(my_provider::Gpt4o::new()) as _),
        "gpt-4o-mini"  => Some(Arc::new(my_provider::Gpt4oMini::new()) as _),
        _              => None,
    }
}

let spec  = IqLadderSpec::from_json(include_str!("ladder.json"))?;
let ladder = spec.bind(resolve)?;
```

## Recommended default ladder

| Tier         | Carryings                                            | Top rung → fallbacks                                         |
|--------------|------------------------------------------------------|--------------------------------------------------------------|
| `Reflex`     | `temperature=0.0`, `max_tokens≈256`, no tools        | `gpt-4o-mini` → `llama3:8b` → `mock`                         |
| `Operator`   | `temperature=0.2`, `max_tokens≈768`, curated tools   | `gpt-4o-mini` → `claude-haiku` → `llama3:8b`                 |
| `Analyst`    | `temperature=0.3`, `max_tokens≈2048`, tools on       | `gpt-4o` → `claude-sonnet` → `llama3:70b`                    |
| `Strategist` | `temperature=0.4`, `max_tokens≈4096`, reflection on  | `gpt-4o` → `claude-sonnet-thinking` → `nemotron-70b`         |
| `Scholar`    | `temperature=0.5`, `max_tokens≈8192`, ensemble on    | `nemotron-ultra-253b` → `gpt-4o` → `claude-opus`             |

## Selection rules

1. If `AgentEnv::current() == Test`, ladder resolution always returns
   a deterministic `MockChatModel` rung — a hard requirement for
   snapshot tests.
2. Otherwise, the resolver probes the profile's tier and then higher
   tiers in order, stopping at the first rung whose `predicate`
   accepts the profile.
3. If no tier match is found, `IqLadder::default_rung` is used.
