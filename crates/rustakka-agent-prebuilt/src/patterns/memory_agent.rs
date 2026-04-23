//! Memory-augmented agent: ReAct with a long-term-memory subgraph.

#![cfg(feature = "memory")]

use rustakka_agent_iq::IqTier;
use rustakka_agent_persona::Persona;

use super::{fresh_blueprint, register_channels, Pattern, RoleTierMap};
use crate::graph::{ChannelSpec, Blueprint, GraphResult};

pub mod channels {
    pub const SCOPE: &str = "memory.scope";
    pub const STORE: &str = "memory.store";
}

/// Scope for long-term memory reads / writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryScope {
    Session,
    User,
    World,
}

impl MemoryScope {
    pub fn as_str(self) -> &'static str {
        match self {
            MemoryScope::Session => "session",
            MemoryScope::User => "user",
            MemoryScope::World => "world",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Builder {
    pub persona: Option<Persona>,
    pub roles: RoleTierMap,
    pub scope: MemoryScope,
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
                .with("agent", IqTier::Analyst)
                .with("memory_read", IqTier::Reflex)
                .with("memory_write", IqTier::Reflex),
            scope: MemoryScope::Session,
        }
    }
    pub fn persona(mut self, p: Persona) -> Self {
        self.persona = Some(p);
        self
    }
    pub fn scope(mut self, s: MemoryScope) -> Self {
        self.scope = s;
        self
    }
}

impl Pattern for Builder {
    fn name(&self) -> &'static str {
        "memory_agent"
    }
    fn channels(&self) -> Vec<ChannelSpec> {
        vec![
            ChannelSpec::last(channels::SCOPE),
            ChannelSpec::last(channels::STORE),
        ]
    }
    fn compile(&self) -> GraphResult<Blueprint> {
        let mut g = fresh_blueprint("memory_agent");
        for n in ["memory_read", "agent", "memory_write"] {
            g.add_node(n);
        }
        g.add_edge("start", "memory_read");
        g.add_edge("memory_read", "agent");
        g.add_edge("agent", "memory_write");
        g.add_edge("memory_write", "end");
        register_channels(&mut g, &self.channels());
        g.channels.insert(
            format!("memory.scope.{}", self.scope.as_str()),
            crate::graph::ChannelKind::LastValue,
        );
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
    fn wires_read_agent_write() {
        let g = Builder::new().compile().unwrap();
        assert!(g.has_edge("memory_read", "agent"));
        assert!(g.has_edge("agent", "memory_write"));
    }

    #[test]
    fn scope_is_stamped_into_channels() {
        let g = Builder::new().scope(MemoryScope::User).compile().unwrap();
        assert!(g.channels.contains_key("memory.scope.user"));
    }
}
