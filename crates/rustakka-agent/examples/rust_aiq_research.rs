//! `rust_aiq_research` — compile an AI-Q-style deep-research graph.
//!
//! ```bash
//! cargo run -p rustakka-agent --example rust_aiq_research --features aiq-research
//! ```

#![cfg(feature = "aiq-research")]

use std::sync::Arc;

use rustakka_agent::prebuilt::aiq_research::{
    create_aiq_research_agent, default_subagent_tiers, AiqResearchOptions, AiqToolkit,
    DefaultCitationVerifier, DefaultReportSanitizer, EnsembleConfig,
};
use rustakka_agent::prelude::*;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let persona = Persona::builder()
        .name("Researcher")
        .role("deep research analyst")
        .values(["rigor", "cite sources"])
        .iq(IqProfile::builder()
            .reasoning_depth(0.85)
            .planning_hops(6)
            .build())
        .build();

    let ladder = IqLadder::default();
    let graph = create_aiq_research_agent(AiqResearchOptions {
        persona: Some(persona),
        ladder,
        allow_deep_path: true,
        hitl_clarifier: true,
        ensemble: Some(EnsembleConfig { parallel_runs: 3 }),
        post_hoc_refiner: true,
        citation_verifier: Arc::new(DefaultCitationVerifier),
        sanitizer: Arc::new(DefaultReportSanitizer),
        tools: AiqToolkit {
            search: vec!["tavily".into()],
            retriever: vec!["pgvector".into()],
            code: vec!["python_sandbox".into()],
        },
    })
    .await?;

    println!("= nodes = {:?}", graph.blueprint.nodes);
    println!(
        "= interrupts before = {:?}",
        graph.blueprint.interrupt_before
    );
    println!(
        "= compiled node count = {}",
        graph.compiled.topology().nodes.len()
    );
    println!("= default subagent tiers =");
    for (name, tier) in default_subagent_tiers() {
        println!("    {name:20} → {tier:?}");
    }
    Ok(())
}
