//! Toolformer / Codex loop: `plan → code → run → observe → repair`, bounded.

#![cfg(feature = "codex-loop")]

use rustakka_agent_iq::IqTier;
use rustakka_agent_persona::Persona;

use super::{fresh_blueprint, register_channels, Pattern, RoleTierMap};
use crate::graph::{ChannelSpec, Blueprint, GraphResult};

pub mod channels {
    pub const DIFF: &str = "codex.diff";
    pub const TEST_LOG: &str = "codex.test_log";
    pub const ATTEMPTS: &str = "codex.attempts";
}

#[derive(Clone, Debug)]
pub struct Builder {
    pub persona: Option<Persona>,
    pub roles: RoleTierMap,
    pub max_attempts: u32,
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
                .with("planner", IqTier::Strategist)
                .with("coder", IqTier::Strategist)
                .with("runner", IqTier::Reflex)
                .with("observer", IqTier::Analyst)
                .with("repair", IqTier::Strategist),
            max_attempts: 3,
        }
    }
    pub fn persona(mut self, p: Persona) -> Self {
        self.persona = Some(p);
        self
    }
    pub fn max_attempts(mut self, n: u32) -> Self {
        self.max_attempts = n;
        self
    }
}

impl Pattern for Builder {
    fn name(&self) -> &'static str {
        "codex_loop"
    }
    fn channels(&self) -> Vec<ChannelSpec> {
        vec![
            ChannelSpec::appended(channels::DIFF),
            ChannelSpec::appended(channels::TEST_LOG),
            ChannelSpec::last(channels::ATTEMPTS),
        ]
    }
    fn compile(&self) -> GraphResult<Blueprint> {
        let mut g = fresh_blueprint("codex_loop");
        for n in ["planner", "coder", "runner", "observer", "repair"] {
            g.add_node(n);
        }
        g.add_edge("start", "planner");
        g.add_edge("planner", "coder");
        g.add_edge("coder", "runner");
        g.add_edge("runner", "observer");
        g.add_edge("observer", "repair");
        g.add_edge("repair", "coder");
        g.add_edge("observer", "end");
        register_channels(&mut g, &self.channels());
        g.recursion_limit = Some(self.max_attempts.saturating_mul(5).max(4));
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
    fn happy_path_has_repair_cycle() {
        let g = Builder::new().compile().unwrap();
        assert!(g.has_edge("repair", "coder"));
        assert!(g.has_edge("observer", "end"));
    }
}
