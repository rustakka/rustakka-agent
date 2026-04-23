//! Phase 5 — reflection node injection + tool biasing.
//!
//! These two middlewares are the mechanism by which EQ / IQ knobs
//! materially change the graph (not just the prompt). They operate on
//! a [`Blueprint`] *before* it is materialized into a real
//! [`CompiledStateGraph`](crate::graph::CompiledStateGraph), so the
//! adjustments are observable by topology tests and by the downstream
//! mermaid/ascii renderers.

use rustakka_agent_eq::{EqProfile, Reflection};
use rustakka_agent_iq::IqProfile;

use crate::graph::{Blueprint, ChannelKind};

/// Splice a `reflect` node into the blueprint when
/// [`EqProfile::reflection_cadence`] requests one.
///
/// Topology after injection (schematic):
///
/// ```text
///   agent → tools → agent → reflect → end
/// ```
pub fn inject_reflection(bp: &mut Blueprint, eq: &EqProfile) {
    let policy = eq.reflection_policy();
    if !policy.insert {
        return;
    }
    bp.add_node("reflect");

    // Re-route `agent → end` through `reflect`.
    bp.edges.retain(|(f, t)| !(f == "agent" && t == "end"));
    if !bp.has_edge("agent", "reflect") {
        bp.add_edge("agent", "reflect");
    }
    if !bp.has_edge("reflect", "end") {
        bp.add_edge("reflect", "end");
    }
    // Annotate the blueprint with *why* the node was injected.
    let reason = match eq.reflection_cadence {
        Reflection::Never => "never",
        Reflection::AfterEachTurn => "after_each_turn",
        Reflection::OnError => "on_error",
        Reflection::OnToolFailure => "on_tool_failure",
    };
    bp.channels
        .insert(format!("reflect.reason.{reason}"), ChannelKind::LastValue);
}

/// Tool-eagerness bias derived from an [`IqProfile`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ToolBias {
    /// When `true`, the `tools_condition` router will default to
    /// `true` (invoke tools) when undecided.
    pub prefer_tools: bool,
    /// When `true`, tool routing is suppressed entirely — the agent
    /// answers directly unless the model explicitly emits a tool call.
    pub suppress_tools: bool,
}

impl ToolBias {
    /// Record the bias on the blueprint as channels so downstream
    /// engine adapters can read it; the authoritative `tools_condition`
    /// wrapping happens when the blueprint is compiled.
    pub fn apply(self, bp: &mut Blueprint) {
        if self.prefer_tools {
            bp.channels
                .insert("router.tool_bias".into(), ChannelKind::LastValue);
        }
        if self.suppress_tools {
            bp.channels
                .insert("router.suppress_tools".into(), ChannelKind::LastValue);
        }
    }
}

/// Derive a [`ToolBias`] from [`IqProfile::tool_eagerness`].
pub fn tool_bias_from_iq(iq: &IqProfile) -> ToolBias {
    let e = iq.tool_eagerness.get();
    if e >= 0.7 {
        ToolBias {
            prefer_tools: true,
            suppress_tools: false,
        }
    } else if e <= 0.05 {
        ToolBias {
            prefer_tools: false,
            suppress_tools: true,
        }
    } else {
        ToolBias::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustakka_agent_eq::{EqProfile, Reflection};

    fn fresh() -> Blueprint {
        let mut bp = Blueprint::new("g");
        bp.add_node("agent");
        bp.add_node("tools");
        bp.add_edge("agent", "tools");
        bp.add_edge("tools", "agent");
        bp.add_edge("agent", "end");
        bp
    }

    #[test]
    fn reflection_never_does_nothing() {
        let mut bp = fresh();
        inject_reflection(&mut bp, &EqProfile::default());
        assert!(!bp.has_node("reflect"));
    }

    #[test]
    fn reflection_reroutes_agent_end_through_reflect() {
        let mut bp = fresh();
        let eq = EqProfile::builder()
            .reflection(Reflection::AfterEachTurn)
            .build();
        inject_reflection(&mut bp, &eq);
        assert!(bp.has_node("reflect"));
        assert!(bp.has_edge("agent", "reflect"));
        assert!(bp.has_edge("reflect", "end"));
        assert!(!bp.has_edge("agent", "end"));
    }

    #[test]
    fn tool_bias_derives_prefer_vs_suppress() {
        let eager = IqProfile::builder().tool_eagerness(0.9).build();
        let cautious = IqProfile::builder().tool_eagerness(0.02).build();
        let mid = IqProfile::builder().tool_eagerness(0.4).build();
        assert!(tool_bias_from_iq(&eager).prefer_tools);
        assert!(tool_bias_from_iq(&cautious).suppress_tools);
        let m = tool_bias_from_iq(&mid);
        assert!(!m.prefer_tools && !m.suppress_tools);
    }
}
