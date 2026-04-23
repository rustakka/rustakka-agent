//! Corrective RAG: `rag → self_grade → (regen | web_search → rag)`.

#![cfg(feature = "crag")]

use rustakka_agent_iq::IqTier;
use rustakka_agent_persona::Persona;

use super::{fresh_blueprint, register_channels, Pattern, RoleTierMap};
use crate::graph::{ChannelSpec, Blueprint, GraphResult};

pub mod channels {
    pub const GRADE: &str = "crag.grade";
    pub const MODE: &str = "crag.mode";
}

#[derive(Clone, Debug)]
pub struct Builder {
    pub persona: Option<Persona>,
    pub roles: RoleTierMap,
    pub max_corrections: u32,
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
                .with("rag", IqTier::Analyst)
                .with("grader", IqTier::Analyst)
                .with("web_search", IqTier::Operator)
                .with("regen", IqTier::Strategist),
            max_corrections: 2,
        }
    }
    pub fn persona(mut self, p: Persona) -> Self {
        self.persona = Some(p);
        self
    }
    pub fn max_corrections(mut self, n: u32) -> Self {
        self.max_corrections = n;
        self
    }
}

impl Pattern for Builder {
    fn name(&self) -> &'static str {
        "crag"
    }
    fn channels(&self) -> Vec<ChannelSpec> {
        vec![
            ChannelSpec::last(channels::GRADE),
            ChannelSpec::last(channels::MODE),
        ]
    }
    fn compile(&self) -> GraphResult<Blueprint> {
        let mut g = fresh_blueprint("crag");
        for n in ["rag", "grader", "web_search", "regen"] {
            g.add_node(n);
        }
        g.add_edge("start", "rag");
        g.add_edge("rag", "grader");
        g.add_edge("grader", "web_search");
        g.add_edge("web_search", "rag");
        g.add_edge("grader", "regen");
        g.add_edge("regen", "end");
        g.add_edge("grader", "end");
        register_channels(&mut g, &self.channels());
        g.recursion_limit = Some(self.max_corrections.saturating_mul(3).max(4));
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
    fn happy_path_has_every_branch() {
        let g = Builder::new().compile().unwrap();
        assert!(g.has_edge("grader", "web_search"));
        assert!(g.has_edge("web_search", "rag"));
        assert!(g.has_edge("grader", "end"));
    }

    #[test]
    fn bound_exhaustion_sets_recursion_limit() {
        let g = Builder::new().max_corrections(1).compile().unwrap();
        assert!(g.recursion_limit.unwrap() >= 4);
    }
}
