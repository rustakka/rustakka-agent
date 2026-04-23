//! Guardrails / policy: `pre_check → agent → post_check` with refusal routes.

#![cfg(feature = "guardrails")]

use rustakka_agent_iq::IqTier;
use rustakka_agent_persona::Persona;

use super::{fresh_blueprint, register_channels, Pattern, RoleTierMap};
use crate::graph::{ChannelSpec, Blueprint, GraphResult};

pub mod channels {
    pub const PREFLIGHT: &str = "guard.preflight";
    pub const POSTFLIGHT: &str = "guard.postflight";
    pub const REFUSAL_REASON: &str = "guard.refusal_reason";
}

#[derive(Clone, Debug)]
pub struct Builder {
    pub persona: Option<Persona>,
    pub roles: RoleTierMap,
    pub refusal_route: bool,
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
                .with("pre_check", IqTier::Reflex)
                .with("agent", IqTier::Analyst)
                .with("post_check", IqTier::Reflex),
            refusal_route: true,
        }
    }
    pub fn persona(mut self, p: Persona) -> Self {
        self.persona = Some(p);
        self
    }
    pub fn refusal_route(mut self, on: bool) -> Self {
        self.refusal_route = on;
        self
    }
}

impl Pattern for Builder {
    fn name(&self) -> &'static str {
        "guardrails"
    }
    fn channels(&self) -> Vec<ChannelSpec> {
        vec![
            ChannelSpec::last(channels::PREFLIGHT),
            ChannelSpec::last(channels::POSTFLIGHT),
            ChannelSpec::last(channels::REFUSAL_REASON),
        ]
    }
    fn compile(&self) -> GraphResult<Blueprint> {
        let mut g = fresh_blueprint("guardrails");
        for n in ["pre_check", "agent", "post_check"] {
            g.add_node(n);
        }
        g.add_edge("start", "pre_check");
        g.add_edge("pre_check", "agent");
        g.add_edge("agent", "post_check");
        g.add_edge("post_check", "end");
        if self.refusal_route {
            g.add_node("refusal");
            g.add_edge("pre_check", "refusal");
            g.add_edge("post_check", "refusal");
            g.add_edge("refusal", "end");
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
    fn happy_path_with_refusal_route() {
        let g = Builder::new().compile().unwrap();
        assert!(g.has_edge("pre_check", "refusal"));
        assert!(g.has_edge("refusal", "end"));
    }
}
