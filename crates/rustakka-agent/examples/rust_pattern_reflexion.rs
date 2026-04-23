//! `rust_pattern_reflexion` — reflexion (act → evaluate → reflect).
//!
//! ```bash
//! cargo run -p rustakka-agent --example rust_pattern_reflexion --features patterns
//! ```

#![cfg(feature = "patterns")]

use rustakka_agent::prebuilt::patterns::reflexion::Builder as Reflexion;
use rustakka_agent::prebuilt::patterns::Pattern;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let graph = Reflexion::new().max_reflections(4).compile()?;
    println!("= nodes = {:?}", graph.nodes);
    println!("= edges = {:?}", graph.edges);
    println!("= recursion limit = {:?}", graph.recursion_limit);
    println!("= channels = {:?}", graph.channels.keys().collect::<Vec<_>>());
    Ok(())
}
