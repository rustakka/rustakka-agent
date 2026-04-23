//! Self-RAG: `generate → verify → regenerate?`.

#![cfg(feature = "self-rag")]

use rustakka_agent_iq::IqTier;
use rustakka_agent_persona::Persona;

use super::{fresh_blueprint, register_channels, Pattern, RoleTierMap};
use crate::graph::{ChannelSpec, Blueprint, GraphResult};

pub mod channels {
    pub const REFLECT_TOKENS: &str = "self_rag.reflect_tokens";
    pub const SUPPORT: &str = "self_rag.support";
}

#[derive(Clone, Debug)]
pub struct Builder {
    pub persona: Option<Persona>,
    pub roles: RoleTierMap,
    pub max_regenerations: u32,
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
                .with("generator", IqTier::Strategist)
                .with("verifier", IqTier::Analyst),
            max_regenerations: 2,
        }
    }
    pub fn persona(mut self, p: Persona) -> Self {
        self.persona = Some(p);
        self
    }
    pub fn max_regenerations(mut self, n: u32) -> Self {
        self.max_regenerations = n;
        self
    }
}

impl Pattern for Builder {
    fn name(&self) -> &'static str {
        "self_rag"
    }
    fn channels(&self) -> Vec<ChannelSpec> {
        vec![
            ChannelSpec::appended(channels::REFLECT_TOKENS),
            ChannelSpec::last(channels::SUPPORT),
        ]
    }
    fn compile(&self) -> GraphResult<Blueprint> {
        let mut g = fresh_blueprint("self_rag");
        for n in ["generator", "verifier"] {
            g.add_node(n);
        }
        g.add_edge("start", "generator");
        g.add_edge("generator", "verifier");
        g.add_edge("verifier", "generator");
        g.add_edge("verifier", "end");
        register_channels(&mut g, &self.channels());
        g.recursion_limit = Some(self.max_regenerations.saturating_mul(2).max(4));
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
    fn happy_path_has_verify_regenerate_loop() {
        let g = Builder::new().compile().unwrap();
        assert!(g.has_edge("verifier", "generator"));
        assert!(g.has_edge("verifier", "end"));
    }
}
