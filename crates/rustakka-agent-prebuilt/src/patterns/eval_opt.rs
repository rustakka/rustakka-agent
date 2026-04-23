//! Evaluator–Optimizer pattern.
//!
//! `generate → evaluate → (accept | optimize → generate)`

#![cfg(feature = "eval-opt")]

use rustakka_agent_iq::IqTier;
use rustakka_agent_persona::Persona;

use super::{fresh_blueprint, register_channels, Pattern, RoleTierMap};
use crate::graph::{ChannelSpec, Blueprint, GraphResult};

pub mod channels {
    pub const SCORE: &str = "eval.score";
    pub const THRESHOLD: &str = "eval.threshold";
    pub const RUBRIC: &str = "eval.rubric";
}

#[derive(Clone, Debug)]
pub struct Builder {
    pub persona: Option<Persona>,
    pub roles: RoleTierMap,
    pub threshold: f32,
    pub max_rounds: u32,
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
                .with("generator", IqTier::Analyst)
                .with("evaluator", IqTier::Analyst)
                .with("optimizer", IqTier::Strategist),
            threshold: 0.8,
            max_rounds: 4,
        }
    }
    pub fn persona(mut self, p: Persona) -> Self {
        self.persona = Some(p);
        self
    }
    pub fn threshold(mut self, v: f32) -> Self {
        self.threshold = v.clamp(0.0, 1.0);
        self
    }
    pub fn max_rounds(mut self, n: u32) -> Self {
        self.max_rounds = n;
        self
    }
}

impl Pattern for Builder {
    fn name(&self) -> &'static str {
        "evaluator_optimizer"
    }
    fn channels(&self) -> Vec<ChannelSpec> {
        vec![
            ChannelSpec::last(channels::SCORE),
            ChannelSpec::last(channels::THRESHOLD),
            ChannelSpec::last(channels::RUBRIC),
        ]
    }
    fn compile(&self) -> GraphResult<Blueprint> {
        let mut g = fresh_blueprint("evaluator_optimizer");
        for n in ["generator", "evaluator", "optimizer"] {
            g.add_node(n);
        }
        g.add_edge("start", "generator");
        g.add_edge("generator", "evaluator");
        g.add_edge("evaluator", "optimizer");
        g.add_edge("optimizer", "generator");
        g.add_edge("evaluator", "end");
        register_channels(&mut g, &self.channels());
        g.recursion_limit = Some(self.max_rounds.saturating_mul(3));
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
    fn happy_path_wires_cycle() {
        let g = Builder::new().compile().unwrap();
        assert!(g.has_edge("optimizer", "generator"));
        assert!(g.has_edge("evaluator", "end"));
    }

    #[test]
    fn threshold_is_clamped() {
        let b = Builder::new().threshold(2.0);
        assert_eq!(b.threshold, 1.0);
    }
}
