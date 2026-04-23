//! Adaptive RAG: `router → {no_retrieve, single_retrieve, multi_hop} → gen`.

#![cfg(feature = "adaptive-rag")]

use rustakka_agent_iq::IqTier;
use rustakka_agent_persona::Persona;

use super::{fresh_blueprint, register_channels, Pattern, RoleTierMap};
use crate::graph::{ChannelSpec, Blueprint, GraphResult};

pub mod channels {
    pub const STRATEGY: &str = "rag.strategy";
}

#[derive(Clone, Debug)]
pub struct Builder {
    pub persona: Option<Persona>,
    pub roles: RoleTierMap,
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
                .with("router", IqTier::Reflex)
                .with("retriever", IqTier::Operator)
                .with("multihop", IqTier::Strategist)
                .with("generator", IqTier::Analyst),
        }
    }
    pub fn persona(mut self, p: Persona) -> Self {
        self.persona = Some(p);
        self
    }
}

impl Pattern for Builder {
    fn name(&self) -> &'static str {
        "adaptive_rag"
    }
    fn channels(&self) -> Vec<ChannelSpec> {
        vec![ChannelSpec::last(channels::STRATEGY)]
    }
    fn compile(&self) -> GraphResult<Blueprint> {
        let mut g = fresh_blueprint("adaptive_rag");
        for n in ["router", "no_retrieve", "single_retrieve", "multi_hop", "generator"] {
            g.add_node(n);
        }
        g.add_edge("start", "router");
        for branch in ["no_retrieve", "single_retrieve", "multi_hop"] {
            g.add_edge("router", branch);
            g.add_edge(branch, "generator");
        }
        g.add_edge("generator", "end");
        register_channels(&mut g, &self.channels());
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
    fn all_strategies_routed_to_generator() {
        let g = Builder::new().compile().unwrap();
        for b in ["no_retrieve", "single_retrieve", "multi_hop"] {
            assert!(g.has_edge(b, "generator"), "missing {b} → generator");
        }
    }
}
