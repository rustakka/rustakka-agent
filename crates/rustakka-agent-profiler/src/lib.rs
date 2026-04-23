//! # rustakka-agent-profiler
//!
//! Micro-benchmark scenarios that measure the hot paths of the agent
//! stack. Designed to be run from the `profiler` binary or imported
//! directly by integration benchmarks.
//!
//! Scenarios follow the Phase-6 plan:
//! - `persona-compile` — round-trip a persona through JSON loaders.
//! - `prompt-render` — render `Persona::to_system_prompt`.
//! - `react-turn` — compile a persona-aware ReAct agent end-to-end.
//!
//! We keep the implementation dependency-light (no `criterion`)
//! because CI just wants stable numbers, not statistical analysis.

use std::time::{Duration, Instant};
use std::sync::Arc;

use rustakka_agent_eq::EqProfile;
use rustakka_agent_iq::IqProfile;
use rustakka_agent_persona::Persona;
use rustakka_agent_prebuilt::{
    create_persona_react_agent,
    graph::mock::echo_provider,
    AgentOptions, ReactAgentOptions,
};
use rustakka_langgraph_providers::prelude::ChatModel as ProviderChatModel;

/// Return a reasonably-realistic sample persona used by every
/// scenario so numbers are comparable.
pub fn sample_persona() -> Persona {
    Persona::builder()
        .name("Prof")
        .role("benchmark persona")
        .iq(IqProfile::builder()
            .reasoning_depth(0.6)
            .planning_hops(3)
            .tool_eagerness(0.5)
            .verbosity(0.3)
            .build())
        .eq(EqProfile::builder().build())
        .values(["accuracy", "clarity"])
        .goals(["Be a stable baseline"])
        .build()
}

/// Named timing result.
#[derive(Clone, Debug)]
pub struct Bench {
    pub name: &'static str,
    pub iterations: u32,
    pub elapsed: Duration,
}

impl Bench {
    pub fn per_iter_us(&self) -> f64 {
        self.elapsed.as_secs_f64() * 1_000_000.0 / f64::from(self.iterations)
    }
}

fn bench<F: FnMut()>(name: &'static str, iterations: u32, mut f: F) -> Bench {
    let t0 = Instant::now();
    for _ in 0..iterations {
        f();
    }
    Bench {
        name,
        iterations,
        elapsed: t0.elapsed(),
    }
}

pub fn persona_compile(iterations: u32) -> Bench {
    let persona = sample_persona();
    let s = serde_json::to_string(&persona).unwrap();
    bench("persona-compile", iterations, || {
        let _ = Persona::from_json(&s).unwrap();
    })
}

pub fn prompt_render(iterations: u32) -> Bench {
    let persona = sample_persona();
    bench("prompt-render", iterations, || {
        let _ = persona.to_system_prompt();
    })
}

pub fn react_turn(iterations: u32) -> Bench {
    let persona = sample_persona();
    let model: Arc<dyn ProviderChatModel> = echo_provider("echo");

    // Construct a tiny runtime once; reuse for every iteration.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    bench("react-turn", iterations, || {
        rt.block_on(async {
            let _ = create_persona_react_agent(
                model.clone(),
                vec![],
                AgentOptions {
                    persona: Some(persona.clone()),
                    react: ReactAgentOptions::default(),
                },
            )
            .await
            .unwrap();
        });
    })
}

/// Run every scenario and return their results.
pub fn run_all(iterations: u32) -> Vec<Bench> {
    vec![
        persona_compile(iterations),
        prompt_render(iterations),
        react_turn(iterations),
    ]
}
