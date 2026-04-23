//! `rust_supervisor_team` — persona-aware supervisor over three
//! discipline agents.
//!
//! ```bash
//! cargo run -p rustakka-agent --example rust_supervisor_team
//! ```

use std::sync::Arc;

use rustakka_agent::prebuilt::graph::mock::echo_provider;
use rustakka_agent::prelude::*;

async fn specialist(
    name: &str,
    role: &str,
    domains: &[&str],
    categories: Vec<String>,
) -> Result<PersonaAgent, Box<dyn std::error::Error>> {
    let persona = Persona::builder()
        .name(name)
        .role(role)
        .knowledge_domains(domains.iter().copied())
        .build();
    let model: Arc<dyn rustakka_langgraph_providers::prelude::ChatModel> = echo_provider("mock");
    Ok(PersonaAgent::new(name, model, persona, vec![], categories).await?)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let boss = specialist("boss", "orchestrator", &["orchestration"], vec!["orchestration".into()]).await?;
    let math = specialist("math", "mathematician", &["mathematics"], vec!["reasoning".into()]).await?;
    let writer = specialist("writer", "prose stylist", &["writing"], vec!["prose".into()]).await?;
    let chef = specialist("chef", "culinary advisor", &["cooking"], vec!["recipes".into()]).await?;

    // Persona-aware routing demonstration (offline).
    let team = [math.clone(), writer.clone(), chef.clone()];
    let pick = persona_based_router(&team, "can you help me with cooking?");
    println!("routing `cooking` → {}", pick.unwrap().name);

    let graph = create_persona_supervisor(
        boss,
        SupervisorRouter::PersonaAware,
        vec![math, writer, chef],
    )
    .await?;
    println!("= supervisor edges = {:?}", graph.blueprint.edges);
    println!("= channels = {:?}", graph.blueprint.channels.keys().collect::<Vec<_>>());
    Ok(())
}
