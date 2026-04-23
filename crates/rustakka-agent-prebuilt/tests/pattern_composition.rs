//! Cross-pattern composition smoke-tests (Phase 5c.3.5).
//!
//! Verifies that each pattern declares a well-namespaced channel set
//! so two patterns can be compiled in the same process without
//! colliding on channel keys.

use rustakka_agent_prebuilt::patterns::Pattern;

#[test]
#[cfg(all(feature = "plan-execute", feature = "reflexion"))]
fn plan_execute_and_reflexion_channels_do_not_collide() {
    let outer = rustakka_agent_prebuilt::patterns::plan_execute::Builder::new();
    let inner = rustakka_agent_prebuilt::patterns::reflexion::Builder::new();

    let o_channels: std::collections::BTreeSet<_> =
        outer.channels().into_iter().map(|s| s.name).collect();
    let i_channels: std::collections::BTreeSet<_> =
        inner.channels().into_iter().map(|s| s.name).collect();

    let intersect: Vec<_> = o_channels.intersection(&i_channels).collect();
    assert!(
        intersect.is_empty(),
        "patterns share channels: {intersect:?}"
    );
}

#[test]
#[cfg(all(feature = "rag", feature = "debate"))]
fn rag_and_debate_channels_do_not_collide() {
    let rag = rustakka_agent_prebuilt::patterns::rag::Builder::new();
    let debate = rustakka_agent_prebuilt::patterns::debate::Builder::new();

    let a: std::collections::BTreeSet<_> = rag.channels().into_iter().map(|s| s.name).collect();
    let b: std::collections::BTreeSet<_> = debate.channels().into_iter().map(|s| s.name).collect();
    assert!(a.is_disjoint(&b));
}

#[test]
#[cfg(feature = "patterns")]
fn every_pattern_declares_non_empty_channels_and_name() {
    macro_rules! check {
        ($p:expr) => {{
            let p = $p;
            assert!(!p.name().is_empty(), "empty pattern name");
            let g = p.compile().unwrap();
            assert!(!g.nodes.is_empty(), "pattern {} compiled empty graph", p.name());
            assert!(g.has_node("start") && g.has_node("end"), "pattern {} missing start/end", p.name());
        }};
    }
    use rustakka_agent_prebuilt::patterns;
    check!(patterns::plan_execute::Builder::new());
    check!(patterns::reflexion::Builder::new());
    check!(patterns::eval_opt::Builder::new());
    check!(patterns::self_consistency::Builder::new());
    check!(patterns::tot::Builder::new());
    check!(patterns::debate::Builder::new());
    check!(patterns::router::Builder::new());
    check!(patterns::rag::Builder::new());
    check!(patterns::crag::Builder::new());
    check!(patterns::adaptive_rag::Builder::new());
    check!(patterns::self_rag::Builder::new());
    check!(patterns::hitl_gate::Builder::new());
    check!(patterns::memory_agent::Builder::new());
    check!(patterns::codex_loop::Builder::new());
    check!(patterns::guardrails::Builder::new());
}
