//! RAG: `retriever → rerank? → grounded_generator → cite_checker`.

#![cfg(feature = "rag")]

use rustakka_agent_iq::IqTier;
use rustakka_agent_persona::Persona;

use super::{fresh_blueprint, register_channels, Pattern, RoleTierMap};
use crate::graph::{ChannelSpec, Blueprint, GraphResult};

pub mod channels {
    pub const QUERY: &str = "rag.query";
    pub const DOCS: &str = "rag.docs";
    pub const CITATIONS: &str = "rag.citations";
}

#[derive(Clone, Debug)]
pub struct Builder {
    pub persona: Option<Persona>,
    pub roles: RoleTierMap,
    pub rerank: bool,
    pub cite_check: bool,
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
                .with("retriever", IqTier::Operator)
                .with("reranker", IqTier::Analyst)
                .with("generator", IqTier::Analyst)
                .with("cite_checker", IqTier::Reflex),
            rerank: true,
            cite_check: true,
        }
    }
    pub fn persona(mut self, p: Persona) -> Self {
        self.persona = Some(p);
        self
    }
    pub fn rerank(mut self, on: bool) -> Self {
        self.rerank = on;
        self
    }
    pub fn cite_check(mut self, on: bool) -> Self {
        self.cite_check = on;
        self
    }
}

impl Pattern for Builder {
    fn name(&self) -> &'static str {
        "rag"
    }
    fn channels(&self) -> Vec<ChannelSpec> {
        vec![
            ChannelSpec::last(channels::QUERY),
            ChannelSpec::appended(channels::DOCS),
            ChannelSpec::appended(channels::CITATIONS),
        ]
    }
    fn compile(&self) -> GraphResult<Blueprint> {
        let mut g = fresh_blueprint("rag");
        g.add_node("retriever");
        g.add_edge("start", "retriever");
        let mut prev = "retriever".to_string();
        if self.rerank {
            g.add_node("reranker");
            g.add_edge(&prev, "reranker");
            prev = "reranker".into();
        }
        g.add_node("generator");
        g.add_edge(&prev, "generator");
        prev = "generator".into();
        if self.cite_check {
            g.add_node("cite_checker");
            g.add_edge(&prev, "cite_checker");
            g.add_edge("cite_checker", "end");
        } else {
            g.add_edge(&prev, "end");
        }
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
    fn full_pipeline_has_every_node() {
        let g = Builder::new().compile().unwrap();
        assert!(g.has_node("retriever") && g.has_node("reranker"));
        assert!(g.has_node("generator") && g.has_node("cite_checker"));
        assert!(g.has_edge("cite_checker", "end"));
    }

    #[test]
    fn minimal_pipeline_without_rerank_or_cite() {
        let g = Builder::new().rerank(false).cite_check(false).compile().unwrap();
        assert!(!g.has_node("reranker") && !g.has_node("cite_checker"));
        assert!(g.has_edge("generator", "end"));
    }
}
