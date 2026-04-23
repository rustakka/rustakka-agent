//! `rust_pattern_rag_suite` — compares the three RAG variants in the
//! pattern catalog (naive, corrective, adaptive, self).
//!
//! ```bash
//! cargo run -p rustakka-agent --example rust_pattern_rag_suite --features patterns
//! ```

#![cfg(feature = "patterns")]

use rustakka_agent::prebuilt::patterns::adaptive_rag::Builder as AdaptiveRag;
use rustakka_agent::prebuilt::patterns::crag::Builder as Crag;
use rustakka_agent::prebuilt::patterns::rag::Builder as Rag;
use rustakka_agent::prebuilt::patterns::self_rag::Builder as SelfRag;
use rustakka_agent::prebuilt::patterns::Pattern;

fn dump<P: Pattern>(p: &P) -> Result<(), Box<dyn std::error::Error>> {
    let g = p.compile()?;
    println!("---- {} ----", p.name());
    println!("nodes   = {:?}", g.nodes);
    println!("edges   = {:?}", g.edges);
    println!("channels = {:?}", g.channels.keys().collect::<Vec<_>>());
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dump(&Rag::new())?;
    dump(&Crag::new())?;
    dump(&AdaptiveRag::new())?;
    dump(&SelfRag::new())?;
    Ok(())
}
