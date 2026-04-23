//! # rustakka-agent
//!
//! Umbrella re-export facade for the `rustakka-agent` workspace. Most
//! callers should import from [`prelude`].
//!
//! ## Feature flags
//!
//! | Feature        | What it enables                                                            |
//! |----------------|----------------------------------------------------------------------------|
//! | `persona`      | `Persona`, `Identity`, `CommunicationStyle`, loaders, validators.          |
//! | `prebuilt`     | `create_persona_react_agent`, `create_persona_supervisor`, `create_persona_swarm`, reflection node injection, tool biasing. |
//! | `patterns`     | The `patterns` catalog (plan-execute, reflexion, RAG, CRAG, …).            |
//! | `aiq-research` | The AI-Q-style deep research graph.                                        |
//! | `omo-harness`  | The Oh-My-OpenAgent-style harness graph.                                   |
//!
//! See [`docs/plan.md`](../../../docs/plan.md) for the design and
//! [`docs/TODO.md`](../../../docs/TODO.md) for the phase checklist.

pub use rustakka_agent_eq as eq;
pub use rustakka_agent_iq as iq;
pub use rustakka_agent_traits as traits;

#[cfg(feature = "persona")]
pub use rustakka_agent_persona as persona;

#[cfg(feature = "prebuilt")]
pub use rustakka_agent_prebuilt as prebuilt;

/// Upstream LLM provider traits + types. Re-exported so callers of
/// this umbrella crate don't have to pull in
/// `rustakka-langgraph-providers` directly for common cases.
#[cfg(feature = "prebuilt")]
pub use rustakka_langgraph_providers as providers;

pub mod prelude {
    pub use crate::eq::{EqProfile, Mood, Reflection};
    pub use crate::iq::{IqLadder, IqProfile, IqTier, ModelRung, TierLadder};
    pub use crate::traits::{AgentEnv, Dimension, Score, Trait, TraitSet};

    #[cfg(feature = "persona")]
    pub use crate::persona::{
        CommunicationStyle, Identity, MemoryPrefs, Persona, Register, SafetyRails,
    };

    #[cfg(feature = "prebuilt")]
    pub use crate::prebuilt::{
        create_persona_react_agent, create_persona_supervisor, create_persona_swarm,
        persona_based_router, AgentOptions, PersonaAgent, PersonaReactAgent, ReactAgentOptions,
        SupervisorRouter,
    };
}
