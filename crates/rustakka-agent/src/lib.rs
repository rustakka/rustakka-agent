//! # rustakka-agent
//!
//! Umbrella re-export facade. Most callers should import from
//! [`prelude`].
//!
//! See `docs/plan.md` for the full design and phase plan. Upcoming
//! crates (`rustakka-agent-prebuilt`, `rustakka-agent-profiler`,
//! PyO3 bindings) will be added here as they land.

pub use rustakka_agent_traits as traits;
pub use rustakka_agent_iq as iq;
pub use rustakka_agent_eq as eq;

#[cfg(feature = "persona")]
pub use rustakka_agent_persona as persona;

pub mod prelude {
    pub use crate::eq::{EqProfile, Mood, Reflection};
    pub use crate::iq::IqProfile;
    pub use crate::traits::{AgentEnv, Dimension, Score, Trait, TraitSet};

    #[cfg(feature = "persona")]
    pub use crate::persona::{
        CommunicationStyle, Identity, MemoryPrefs, Persona, Register, SafetyRails,
    };
}
