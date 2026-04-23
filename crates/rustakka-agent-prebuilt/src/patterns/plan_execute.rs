//! Plan-and-Execute pattern.
//!
//! `planner → executor[*] → replanner? → END`

#![cfg(feature = "plan-execute")]

use rustakka_agent_iq::IqTier;
use rustakka_agent_persona::Persona;

use super::{fresh_blueprint, register_channels, Pattern, RoleTierMap};
use crate::graph::{ChannelSpec, Blueprint, GraphResult};

pub mod channels {
    pub const PLAN: &str = "plan";
    pub const PLAN_STEPS: &str = "plan.steps";
    pub const PLAN_CURSOR: &str = "plan.cursor";
    pub const PLAN_REVISIONS: &str = "plan.revisions";
}

#[derive(Clone, Debug, Default)]
pub struct Builder {
    pub persona: Option<Persona>,
    pub roles: RoleTierMap,
    pub replanner: bool,
    pub max_steps: u32,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            replanner: true,
            max_steps: 10,
            roles: RoleTierMap::default()
                .with("planner", IqTier::Strategist)
                .with("executor", IqTier::Analyst)
                .with("replanner", IqTier::Strategist),
            ..Default::default()
        }
    }
    pub fn persona(mut self, p: Persona) -> Self {
        self.persona = Some(p);
        self
    }
    pub fn replanner(mut self, on: bool) -> Self {
        self.replanner = on;
        self
    }
    pub fn max_steps(mut self, n: u32) -> Self {
        self.max_steps = n;
        self
    }
    pub fn roles(mut self, r: RoleTierMap) -> Self {
        self.roles = r;
        self
    }
}

impl Pattern for Builder {
    fn name(&self) -> &'static str {
        "plan_execute"
    }
    fn channels(&self) -> Vec<ChannelSpec> {
        vec![
            ChannelSpec::last(channels::PLAN),
            ChannelSpec::appended(channels::PLAN_STEPS),
            ChannelSpec::last(channels::PLAN_CURSOR),
            ChannelSpec::appended(channels::PLAN_REVISIONS),
        ]
    }
    fn compile(&self) -> GraphResult<Blueprint> {
        let mut g = fresh_blueprint("plan_execute");
        g.add_node("planner");
        g.add_node("executor");
        g.add_edge("start", "planner");
        g.add_edge("planner", "executor");
        g.add_edge("executor", "executor");
        if self.replanner {
            g.add_node("replanner");
            g.add_edge("executor", "replanner");
            g.add_edge("replanner", "executor");
            g.add_edge("replanner", "end");
        } else {
            g.add_edge("executor", "end");
        }
        register_channels(&mut g, &self.channels());
        if let Some(p) = &self.persona {
            g.system_prompt = Some(p.to_system_prompt()).filter(|s| !s.is_empty());
        }
        g.recursion_limit = Some(self.max_steps.max(4));
        Ok(g)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_wires_planner_executor_replanner() {
        let g = Builder::new().compile().unwrap();
        assert!(g.has_node("planner") && g.has_node("executor") && g.has_node("replanner"));
        assert!(g.has_edge("planner", "executor"));
        assert!(g.has_edge("replanner", "end"));
    }

    #[test]
    fn no_replanner_goes_straight_to_end() {
        let g = Builder::new().replanner(false).compile().unwrap();
        assert!(!g.has_node("replanner"));
        assert!(g.has_edge("executor", "end"));
    }
}
