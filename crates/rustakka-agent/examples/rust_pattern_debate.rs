//! `rust_pattern_debate` — proposer/critic/judge multi-round debate.
//!
//! ```bash
//! cargo run -p rustakka-agent --example rust_pattern_debate --features patterns
//! ```

#![cfg(feature = "patterns")]

use rustakka_agent::prebuilt::patterns::debate::Builder as Debate;
use rustakka_agent::prebuilt::patterns::Pattern;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let graph = Debate::new().proposers(3).critics(2).rounds(3).compile()?;
    println!("= nodes = {:?}", graph.nodes);
    println!("= edges = {:?}", graph.edges);
    println!("= channels = {:?}", graph.channels.keys().collect::<Vec<_>>());
    Ok(())
}
