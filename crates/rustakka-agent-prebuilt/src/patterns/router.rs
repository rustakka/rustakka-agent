//! Router / Mixture-of-Experts: `classifier → {expert_i}`.

#![cfg(feature = "router")]

use rustakka_agent_iq::IqTier;
use rustakka_agent_persona::Persona;

use super::{fresh_blueprint, register_channels, Pattern, RoleTierMap};
use crate::graph::{ChannelSpec, Blueprint, GraphResult};

pub mod channels {
    pub const INTENT: &str = "router.intent";
    pub const SELECTED: &str = "router.selected";
    pub const CONFIDENCE: &str = "router.confidence";
}

#[derive(Clone, Debug)]
pub struct Builder {
    pub persona: Option<Persona>,
    pub roles: RoleTierMap,
    pub experts: Vec<String>,
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
                .with("classifier", IqTier::Reflex)
                .with("expert", IqTier::Analyst),
            experts: Vec::new(),
        }
    }
    pub fn persona(mut self, p: Persona) -> Self {
        self.persona = Some(p);
        self
    }
    pub fn expert(mut self, name: impl Into<String>) -> Self {
        self.experts.push(name.into());
        self
    }
}

impl Pattern for Builder {
    fn name(&self) -> &'static str {
        "router"
    }
    fn channels(&self) -> Vec<ChannelSpec> {
        vec![
            ChannelSpec::last(channels::INTENT),
            ChannelSpec::last(channels::SELECTED),
            ChannelSpec::last(channels::CONFIDENCE),
        ]
    }
    fn compile(&self) -> GraphResult<Blueprint> {
        let mut g = fresh_blueprint("router");
        g.add_node("classifier");
        g.add_edge("start", "classifier");
        for e in &self.experts {
            g.add_node(e);
            g.add_edge("classifier", e);
            g.add_edge(e, "end");
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
    fn wires_classifier_to_each_expert() {
        let g = Builder::new().expert("a").expert("b").compile().unwrap();
        assert!(g.has_edge("classifier", "a"));
        assert!(g.has_edge("b", "end"));
    }

    #[test]
    fn bound_exhaustion_empty_experts_only_has_classifier() {
        let g = Builder::new().compile().unwrap();
        assert!(g.has_node("classifier"));
        assert_eq!(g.nodes.len(), 3); // start, end, classifier
    }
}
