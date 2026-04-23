//! `rust_omo_harness` — compile an Oh-My-OpenAgent-style harness.
//!
//! ```bash
//! cargo run -p rustakka-agent --example rust_omo_harness --features omo-harness
//! ```

#![cfg(feature = "omo-harness")]

use std::sync::Arc;

use rustakka_agent::prebuilt::graph::mock::echo_provider;
use rustakka_agent::prebuilt::omo_harness::{
    create_omo_harness, default_disciplines, HashlineMode, InMemoryStore, OmoHarnessOptions,
};
use rustakka_agent::prelude::*;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ladder = IqLadder::default();
    let model: Arc<dyn rustakka_langgraph_providers::prelude::ChatModel> = echo_provider("mock");

    let orchestrator_persona = Persona::builder()
        .name("sisyphus")
        .role("orchestrator")
        .knowledge_domains(["orchestration"])
        .build();
    let orchestrator = PersonaAgent::new(
        "sisyphus",
        model.clone(),
        orchestrator_persona,
        vec![],
        vec!["orchestration".into()],
    )
    .await?;

    let disciplines = default_disciplines(&ladder, model).await?;

    let graph = create_omo_harness(OmoHarnessOptions {
        ladder,
        orchestrator,
        disciplines,
        boulder_store: Some(Arc::new(InMemoryStore::new())),
        hashline: HashlineMode::Enforce,
        default_set: true,
    })
    .await?;

    println!("= nodes = {:?}", graph.blueprint.nodes);
    println!("= channels =");
    for k in graph.blueprint.channels.keys() {
        println!("    {k}");
    }
    Ok(())
}
