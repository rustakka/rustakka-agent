//! Debate / Jury pattern: `proposer[*] → critic[*] → judge`, multi-round.

#![cfg(feature = "debate")]

use rustakka_agent_iq::IqTier;
use rustakka_agent_persona::Persona;

use super::{fresh_blueprint, register_channels, Pattern, RoleTierMap};
use crate::graph::{ChannelSpec, Blueprint, GraphResult};

pub mod channels {
    pub const ROUNDS: &str = "debate.rounds";
    pub const ARGUMENTS: &str = "debate.arguments";
    pub const VERDICT: &str = "debate.verdict";
}

#[derive(Clone, Debug)]
pub struct Builder {
    pub persona: Option<Persona>,
    pub roles: RoleTierMap,
    pub proposers: u32,
    pub critics: u32,
    pub rounds: u32,
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
                .with("proposer", IqTier::Strategist)
                .with("critic", IqTier::Strategist)
                .with("judge", IqTier::Strategist),
            proposers: 2,
            critics: 2,
            rounds: 2,
        }
    }
    pub fn persona(mut self, p: Persona) -> Self {
        self.persona = Some(p);
        self
    }
    pub fn rounds(mut self, n: u32) -> Self {
        self.rounds = n.max(1);
        self
    }
    pub fn proposers(mut self, n: u32) -> Self {
        self.proposers = n.max(1);
        self
    }
    pub fn critics(mut self, n: u32) -> Self {
        self.critics = n.max(1);
        self
    }
}

impl Pattern for Builder {
    fn name(&self) -> &'static str {
        "debate"
    }
    fn channels(&self) -> Vec<ChannelSpec> {
        vec![
            ChannelSpec::last(channels::ROUNDS),
            ChannelSpec::appended(channels::ARGUMENTS),
            ChannelSpec::last(channels::VERDICT),
        ]
    }
    fn compile(&self) -> GraphResult<Blueprint> {
        let mut g = fresh_blueprint("debate");
        g.add_node("judge");
        for i in 0..self.proposers {
            let p = format!("proposer_{i}");
            g.add_node(&p);
            g.add_edge("start", &p);
            for j in 0..self.critics {
                let c = format!("critic_{j}");
                if !g.has_node(&c) {
                    g.add_node(&c);
                }
                g.add_edge(&p, &c);
                g.add_edge(&c, "judge");
            }
        }
        g.add_edge("judge", "end");
        register_channels(&mut g, &self.channels());
        g.recursion_limit = Some(self.rounds.saturating_mul(3).max(4));
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
    fn happy_path_wires_proposers_critics_judge() {
        let g = Builder::new().proposers(2).critics(2).compile().unwrap();
        assert!(g.has_node("proposer_0") && g.has_node("critic_1"));
        assert!(g.has_edge("critic_0", "judge"));
    }

    #[test]
    fn rounds_floor_at_one() {
        assert_eq!(Builder::new().rounds(0).rounds, 1);
    }
}
