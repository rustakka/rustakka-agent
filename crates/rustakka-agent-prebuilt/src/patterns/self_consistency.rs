//! Self-Consistency: `fan_out[N] → majority/scorer → aggregate`.

#![cfg(feature = "self-consistency")]

use rustakka_agent_iq::IqTier;
use rustakka_agent_persona::Persona;

use super::{fresh_blueprint, register_channels, Pattern, RoleTierMap};
use crate::graph::{ChannelSpec, Blueprint, GraphResult};

pub mod channels {
    pub const SAMPLES: &str = "sc.samples";
    pub const VOTES: &str = "sc.votes";
    pub const WINNER: &str = "sc.winner";
}

#[derive(Clone, Debug)]
pub struct Builder {
    pub persona: Option<Persona>,
    pub roles: RoleTierMap,
    pub samples: u32,
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
                .with("aggregator", IqTier::Reflex),
            samples: 5,
        }
    }
    pub fn persona(mut self, p: Persona) -> Self {
        self.persona = Some(p);
        self
    }
    pub fn samples(mut self, n: u32) -> Self {
        self.samples = n.max(1);
        self
    }
}

impl Pattern for Builder {
    fn name(&self) -> &'static str {
        "self_consistency"
    }
    fn channels(&self) -> Vec<ChannelSpec> {
        vec![
            ChannelSpec::appended(channels::SAMPLES),
            ChannelSpec::appended(channels::VOTES),
            ChannelSpec::last(channels::WINNER),
        ]
    }
    fn compile(&self) -> GraphResult<Blueprint> {
        let mut g = fresh_blueprint("self_consistency");
        g.add_node("aggregator");
        for i in 0..self.samples {
            let name = format!("gen_{i}");
            g.add_node(&name);
            g.add_edge("start", &name);
            g.add_edge(&name, "aggregator");
        }
        g.add_edge("aggregator", "end");
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
    fn happy_path_spawns_n_generators() {
        let g = Builder::new().samples(3).compile().unwrap();
        assert!(g.has_node("gen_0") && g.has_node("gen_2"));
        assert!(g.has_edge("gen_0", "aggregator"));
    }

    #[test]
    fn samples_clamp_to_at_least_one() {
        let b = Builder::new().samples(0);
        assert_eq!(b.samples, 1);
    }
}
