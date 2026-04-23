//! `rust_persona_react` — create a persona-aware ReAct agent.
//!
//! Run with:
//! ```bash
//! cargo run -p rustakka-agent --example rust_persona_react
//! ```
//!
//! Illustrates Phase 3: `create_persona_react_agent` merging a
//! persona's system prompt + IQ knobs into the underlying ReAct
//! graph.

use std::sync::Arc;

use rustakka_agent::prebuilt::graph::mock::echo_provider;
use rustakka_agent::prebuilt::graph::Tool;
use rustakka_agent::prelude::*;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let persona = Persona::builder()
        .name("Ada")
        .role("math tutor")
        .bio("Patient, rigorous, encourages durable intuition.")
        .values(["clarity", "accuracy"])
        .goals(["help the learner build intuition"])
        .iq(IqProfile::builder()
            .reasoning_depth(0.7)
            .planning_hops(3)
            .preferred_model("gpt-4o")
            .temperature(0.3)
            .verbosity(0.4)
            .build())
        .eq(EqProfile::builder()
            .empathy(0.8)
            .warmth(0.7)
            .reflection(Reflection::OnError)
            .build())
        .knowledge_domains(["mathematics", "pedagogy"])
        .build();

    let model: Arc<dyn rustakka_agent::providers::prelude::ChatModel> = echo_provider("mock");
    let tools = vec![
        Tool::new("search", "web search"),
        Tool::new("calc", "deterministic calculator"),
    ];
    let agent = create_persona_react_agent(
        model,
        tools,
        AgentOptions {
            persona: Some(persona),
            react: ReactAgentOptions::default(),
        },
    )
    .await?;

    println!(
        "= system prompt =\n{}\n",
        agent.graph.blueprint.system_prompt.as_deref().unwrap_or_default()
    );
    println!("= call options = {:?}", agent.graph.call_options);
    println!(
        "= recursion limit = {:?}",
        agent.graph.blueprint.recursion_limit
    );
    println!("= blueprint nodes = {:?}", agent.graph.blueprint.nodes);
    println!(
        "= compiled node count = {}",
        agent.graph.compiled.topology().nodes.len()
    );
    println!("= mermaid =\n{}", agent.graph.draw_mermaid());
    Ok(())
}
