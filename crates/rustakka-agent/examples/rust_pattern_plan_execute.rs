//! `rust_pattern_plan_execute` — compile the plan-and-execute pattern.
//!
//! ```bash
//! cargo run -p rustakka-agent --example rust_pattern_plan_execute --features patterns
//! ```

#![cfg(feature = "patterns")]

use rustakka_agent::prebuilt::patterns::plan_execute::Builder as PlanExecute;
use rustakka_agent::prebuilt::patterns::Pattern;
use rustakka_agent::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let persona = Persona::builder()
        .name("Planner")
        .role("structured problem solver")
        .iq(IqProfile::builder().planning_hops(5).build())
        .build();

    let graph = PlanExecute::new()
        .persona(persona)
        .replanner(true)
        .max_steps(8)
        .compile()?;

    println!("pattern = {}", PlanExecute::new().name());
    println!("= nodes = {:?}", graph.nodes);
    println!("= edges = {:?}", graph.edges);
    println!("= channels = {:?}", graph.channels.keys().collect::<Vec<_>>());
    Ok(())
}
