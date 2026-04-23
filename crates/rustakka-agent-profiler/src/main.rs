//! Profiler CLI. Runs each scenario and prints per-iteration timings.
//!
//! ```text
//! $ cargo run -p rustakka-agent-profiler -- 10000
//! persona-compile   : 10000 iter,  12.34 µs/iter
//! prompt-render     : 10000 iter,   5.67 µs/iter
//! react-turn        : 10000 iter,  42.00 µs/iter
//! ```

fn main() {
    let iterations = std::env::args()
        .nth(1)
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(1_000);

    for b in rustakka_agent_profiler::run_all(iterations) {
        println!(
            "{name:<18}: {iter} iter, {us:>8.2} µs/iter",
            name = b.name,
            iter = b.iterations,
            us = b.per_iter_us()
        );
    }
}
