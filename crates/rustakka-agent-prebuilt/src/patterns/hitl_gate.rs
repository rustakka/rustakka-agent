//! Human-in-the-loop gate: `propose → interrupt → resume`.
//!
//! The gate is expressed via the upstream `interrupt_before`
//! mechanism — the pattern itself never blocks the runtime.

#![cfg(feature = "hitl")]

use rustakka_agent_iq::IqTier;
use rustakka_agent_persona::Persona;

use super::{fresh_blueprint, register_channels, Pattern, RoleTierMap};
use crate::graph::{Blueprint, ChannelSpec, GraphResult};

pub mod channels {
    pub const AWAITING: &str = "hitl.awaiting";
    pub const DECISION: &str = "hitl.decision";
    pub const PAYLOAD: &str = "hitl.payload";
}

#[derive(Clone, Debug)]
pub struct Builder {
    pub persona: Option<Persona>,
    pub roles: RoleTierMap,
    pub gate_node: String,
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
            roles: RoleTierMap::default().with("proposer", IqTier::Analyst),
            gate_node: "gate".into(),
        }
    }
    pub fn persona(mut self, p: Persona) -> Self {
        self.persona = Some(p);
        self
    }
    pub fn gate_node(mut self, n: impl Into<String>) -> Self {
        self.gate_node = n.into();
        self
    }
}

impl Pattern for Builder {
    fn name(&self) -> &'static str {
        "hitl_gate"
    }
    fn channels(&self) -> Vec<ChannelSpec> {
        vec![
            ChannelSpec::last(channels::AWAITING),
            ChannelSpec::last(channels::DECISION),
            ChannelSpec::last(channels::PAYLOAD),
        ]
    }
    fn compile(&self) -> GraphResult<Blueprint> {
        let mut g = fresh_blueprint("hitl_gate");
        g.add_node("proposer");
        g.add_node(&self.gate_node);
        g.add_edge("start", "proposer");
        g.add_edge("proposer", &self.gate_node);
        g.add_edge(&self.gate_node, "end");
        g.interrupt_before.push(self.gate_node.clone());
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
    fn interrupt_before_registers_gate() {
        let g = Builder::new().compile().unwrap();
        assert!(g.interrupt_before.contains(&"gate".to_string()));
    }
}
