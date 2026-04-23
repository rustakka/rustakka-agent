//! Tree-of-Thoughts / LATS — bounded MCTS-style search.

#![cfg(feature = "tree-of-thought")]

use rustakka_agent_iq::IqTier;
use rustakka_agent_persona::Persona;

use super::{fresh_blueprint, register_channels, Pattern, RoleTierMap};
use crate::graph::{ChannelSpec, Blueprint, GraphResult};

pub mod channels {
    pub const FRONTIER: &str = "tot.frontier";
    pub const SCORES: &str = "tot.scores";
    pub const BUDGET: &str = "tot.budget";
}

#[derive(Clone, Debug)]
pub struct Builder {
    pub persona: Option<Persona>,
    pub roles: RoleTierMap,
    pub depth_budget: u32,
    pub beam_width: u32,
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
                .with("expander", IqTier::Strategist)
                .with("evaluator", IqTier::Analyst)
                .with("selector", IqTier::Analyst),
            depth_budget: 4,
            beam_width: 3,
        }
    }
    pub fn persona(mut self, p: Persona) -> Self {
        self.persona = Some(p);
        self
    }
    pub fn depth(mut self, n: u32) -> Self {
        self.depth_budget = n;
        self
    }
    pub fn beam_width(mut self, n: u32) -> Self {
        self.beam_width = n.max(1);
        self
    }
}

impl Pattern for Builder {
    fn name(&self) -> &'static str {
        "tot"
    }
    fn channels(&self) -> Vec<ChannelSpec> {
        vec![
            ChannelSpec::appended(channels::FRONTIER),
            ChannelSpec::appended(channels::SCORES),
            ChannelSpec::last(channels::BUDGET),
        ]
    }
    fn compile(&self) -> GraphResult<Blueprint> {
        let mut g = fresh_blueprint("tot");
        for n in ["expander", "evaluator", "selector"] {
            g.add_node(n);
        }
        g.add_edge("start", "expander");
        g.add_edge("expander", "evaluator");
        g.add_edge("evaluator", "selector");
        g.add_edge("selector", "expander");
        g.add_edge("selector", "end");
        register_channels(&mut g, &self.channels());
        g.recursion_limit = Some(self.depth_budget.saturating_mul(self.beam_width).max(4));
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
    fn happy_path_closes_cycle() {
        let g = Builder::new().compile().unwrap();
        assert!(g.has_edge("selector", "expander"));
        assert!(g.has_edge("selector", "end"));
    }

    #[test]
    fn beam_width_is_at_least_one() {
        let b = Builder::new().beam_width(0);
        assert_eq!(b.beam_width, 1);
    }
}
