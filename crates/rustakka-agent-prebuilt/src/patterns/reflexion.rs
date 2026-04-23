//! Reflexion pattern: `act → evaluate → reflect → act`, bounded.

#![cfg(feature = "reflexion")]

use rustakka_agent_iq::IqTier;
use rustakka_agent_persona::Persona;

use super::{fresh_blueprint, register_channels, Pattern, RoleTierMap};
use crate::graph::{ChannelSpec, Blueprint, GraphResult};

pub mod channels {
    pub const MEMORY: &str = "reflexion.memory";
    pub const CRITIQUE: &str = "reflexion.critique";
    pub const ATTEMPTS: &str = "reflexion.attempts";
}

#[derive(Clone, Debug)]
pub struct Builder {
    pub persona: Option<Persona>,
    pub roles: RoleTierMap,
    pub max_reflections: u32,
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

impl Builder {
    pub fn new() -> Self {
        Self {
            persona: None,
            roles: RoleTierMap::default()
                .with("actor", IqTier::Analyst)
                .with("evaluator", IqTier::Analyst)
                .with("reflector", IqTier::Strategist),
            max_reflections: 3,
        }
    }
    pub fn persona(mut self, p: Persona) -> Self {
        self.persona = Some(p);
        self
    }
    pub fn max_reflections(mut self, n: u32) -> Self {
        self.max_reflections = n;
        self
    }
}

impl Pattern for Builder {
    fn name(&self) -> &'static str {
        "reflexion"
    }
    fn channels(&self) -> Vec<ChannelSpec> {
        vec![
            ChannelSpec::appended(channels::MEMORY),
            ChannelSpec::last(channels::CRITIQUE),
            ChannelSpec::last(channels::ATTEMPTS),
        ]
    }
    fn compile(&self) -> GraphResult<Blueprint> {
        let mut g = fresh_blueprint("reflexion");
        for n in ["actor", "evaluator", "reflector"] {
            g.add_node(n);
        }
        g.add_edge("start", "actor");
        g.add_edge("actor", "evaluator");
        g.add_edge("evaluator", "reflector");
        g.add_edge("reflector", "actor");
        g.add_edge("evaluator", "end");
        register_channels(&mut g, &self.channels());
        g.recursion_limit = Some(self.max_reflections.saturating_mul(3));
        if let Some(p) = &self.persona {
            g.system_prompt = Some(p.to_system_prompt()).filter(|s| !s.is_empty());
        }
        Ok(g)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path() {
        let g = Builder::new().compile().unwrap();
        assert!(g.has_edge("actor", "evaluator") && g.has_edge("reflector", "actor"));
    }

    #[test]
    fn bound_exhaustion_caps_recursion() {
        let g = Builder::new().max_reflections(2).compile().unwrap();
        assert_eq!(g.recursion_limit, Some(6));
    }
}
