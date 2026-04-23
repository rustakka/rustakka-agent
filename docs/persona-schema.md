# Persona schema

Authoritative reference for the `Persona` JSON / YAML / TOML document
understood by [`rustakka-agent-persona`](../crates/rustakka-agent-persona/).
Every field is optional; `Persona::default()` is a no-op overlay that
leaves upstream `rustakka-langgraph` behavior unchanged.

## Top-level shape (JSON)

```json
{
  "identity":          { "name": "Ada", "role": "tutor", "bio": "…", "pronouns": "she/her" },
  "iq": {
    "reasoning_depth": 0.7,
    "planning_hops":   3,
    "tool_eagerness":  0.4,
    "verbosity":       0.3,
    "preferred_model": "gpt-4o",
    "temperature":     0.3,
    "pinned_tier":     "Analyst",
    "extra":           {}
  },
  "eq": {
    "empathy":            0.8,
    "warmth":             0.7,
    "assertiveness":      0.4,
    "humor":              0.2,
    "mood":               "Calm",
    "reflection_cadence": "OnError",
    "extra":              {}
  },
  "values":            ["clarity", "accuracy"],
  "goals":             ["Help learners build durable intuition"],
  "style": {
    "formality":         0.4,
    "register":          "Socratic",
    "language":          "en",
    "signature_phrases": ["Let's see..."]
  },
  "knowledge_domains": ["mathematics", "pedagogy"],
  "taboos":            ["mock the learner"],
  "memory":  { "long_term": false, "summarize_after_turns": 12, "scope": "user" },
  "safety":  { "deny_topics": ["personal medical advice"], "refusal_style": "kind and brief", "deny_all": false },
  "custom":  {}
}
```

## Loaders

| Format | Loader                         | Feature flag              |
|--------|--------------------------------|---------------------------|
| JSON   | `Persona::from_json(&str)`     | always on                 |
| YAML   | `Persona::from_yaml(&str)`     | `yaml` (enabled by default) |
| TOML   | `Persona::from_toml(&str)`     | `toml` (enabled by default) |

All loaders call `env_validate()`, which runs the strict validator in
`AgentEnv::Test` / `AgentEnv::Prod` and warns-only in `AgentEnv::Dev`.

## Fields

### `identity`

| Field      | Type                | Notes                                    |
|------------|---------------------|------------------------------------------|
| `name`     | `Option<String>`    | Displayed in the rendered prompt.        |
| `pronouns` | `Option<String>`    | Rendered only when present.              |
| `role`     | `Option<String>`    | Short role label, e.g. `"tutor"`.        |
| `bio`      | `Option<String>`    | 1–3-sentence backstory.                  |

### `iq` (see also `docs/iq-ladders.md`)

| Field            | Type             | Range           |
|------------------|------------------|-----------------|
| `reasoning_depth`| `Score` (f32)    | `[0.0, 1.0]`    |
| `planning_hops`  | `u32`            | `0..∞` (soft-capped at 10 for tier inference) |
| `tool_eagerness` | `Score` (f32)    | `[0.0, 1.0]`    |
| `verbosity`      | `Score` (f32)    | `[0.0, 1.0]`    |
| `preferred_model`| `Option<String>` | provider-owned  |
| `temperature`    | `Option<f32>`    | `[0.0, 2.0]`    |
| `pinned_tier`    | `Option<IqTier>` | `Reflex … Scholar` |
| `extra`          | `TraitSet`       | free-form       |

### `eq`

| Field                | Type           | Values                                          |
|----------------------|----------------|-------------------------------------------------|
| `empathy`, `warmth`, `assertiveness`, `humor` | `Score` | `[0.0, 1.0]` |
| `mood`               | `Mood`         | `Neutral \| Upbeat \| Calm \| Serious \| Playful \| Stoic` |
| `reflection_cadence` | `Reflection`   | `Never \| AfterEachTurn \| OnError \| OnToolFailure` |
| `extra`              | `TraitSet`     | free-form                                       |

### `style`

| Field                | Type                | Notes                                         |
|----------------------|---------------------|-----------------------------------------------|
| `formality`          | `Score`             | `0.0` = casual, `1.0` = formal               |
| `register`           | `Register`          | `Plain \| Technical \| Socratic \| Casual \| Formal` |
| `language`           | `Option<String>`    | BCP-47 hint (defaults to English)             |
| `signature_phrases`  | `Vec<String>`       | Short, on-brand phrases to seed the model     |

### `memory`

| Field                 | Type             | Notes                                      |
|-----------------------|------------------|--------------------------------------------|
| `long_term`           | `bool`           | Hints at whether a memory subgraph is used |
| `summarize_after_turns` | `Option<u32>` | When to emit summary side-effects          |
| `scope`               | `Option<String>` | `"session" \| "user" \| "world" \| …`       |

### `safety`

| Field           | Type             | Notes                                              |
|-----------------|------------------|----------------------------------------------------|
| `deny_topics`   | `Vec<String>`    | Topics the agent must refuse                       |
| `refusal_style` | `Option<String>` | Short directive for refusal wording                |
| `deny_all`      | `bool`           | When `true`, all `taboos` exceptions are invalid   |

### `custom`

Free-form `TraitSet` — a `BTreeMap<String, Trait>` projection that
lets callers extend the schema without breaking changes. Traits are
rendered in sorted order for prompt determinism.

## Validation

`Persona::validate` returns `Ok(warnings)` or `Err(PersonaError)`.
Hard errors:

- `safety.deny_all = true` with a non-empty `taboos`.
- `iq.temperature` outside `[0.0, 2.0]` or non-finite.
- Any serde parse error (`Json` / `Yaml` / `TomlDe`).

Soft warnings:

- `EmptyIdentity` — no `name` / `role` / `bio`.
- `ContradictoryValuesAndGoals` — a `value` appears in `taboos`.
- `ExcessiveReflection` — `reflection_cadence = AfterEachTurn` with
  `planning_hops > 8` (likely to blow the recursion budget).

## Prompt rendering

`Persona::to_system_prompt()` produces a deterministic, locale-stable
system-prompt fragment. The sections are emitted in this order (each
omitted when empty):

1. `Identity:`
2. `Values:` (sorted)
3. `Goals:` (caller order preserved)
4. `Cognitive stance:` (from `IqProfile::to_prompt_fragment`)
5. `Emotional stance:` (from `EqProfile::to_prompt_fragment`)
6. `Communication style:`
7. `Knowledge domains:` (sorted)
8. `Safety rails:`
9. `Traits:` (from `custom.to_prompt_fragment`)

`Persona::role_fragment("planner" | "critic" | "synthesizer" |
"researcher" | "retriever")` appends a per-role directive so a single
persona can modulate every node in a pattern consistently.
