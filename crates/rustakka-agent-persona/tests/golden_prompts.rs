//! Golden-file snapshot tests for `Persona::to_system_prompt`.
//!
//! These lock the deterministic prompt output so future refactors
//! can't silently drift. To (re)generate fixtures after an
//! intentional change, run:
//!
//! ```bash
//! RUSTAKKA_UPDATE_GOLDENS=1 cargo test -p rustakka-agent-persona --test golden_prompts
//! ```

use std::path::{Path, PathBuf};

use rustakka_agent_eq::{EqProfile, Mood, Reflection};
use rustakka_agent_iq::IqProfile;
use rustakka_agent_persona::{CommunicationStyle, Persona, Register, SafetyRails};
use rustakka_agent_traits::Score;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn assert_golden(name: &str, actual: &str) {
    let path = fixtures_dir().join(format!("{name}.txt"));
    let update = std::env::var("RUSTAKKA_UPDATE_GOLDENS").is_ok();
    if update || !path.exists() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read golden {}: {e}", path.display()));
    assert_eq!(
        actual,
        expected,
        "\nGolden prompt for `{name}` drifted.\n\
         Path: {path}\n\
         Re-run with RUSTAKKA_UPDATE_GOLDENS=1 if the change is intentional.",
        path = path.display()
    );
}

#[test]
fn tutor_persona_prompt_is_locked() {
    let persona = Persona::builder()
        .name("Ada")
        .role("math tutor")
        .bio("Patient, rigorous, encourages durable intuition.")
        .values(["clarity", "accuracy", "encouragement"])
        .goals(["Help learners build durable intuition"])
        .iq(IqProfile::builder()
            .reasoning_depth(0.7)
            .planning_hops(3)
            .tool_eagerness(0.4)
            .verbosity(0.3)
            .build())
        .eq(EqProfile::builder()
            .empathy(0.8)
            .warmth(0.7)
            .mood(Mood::Calm)
            .reflection(Reflection::OnError)
            .build())
        .style(CommunicationStyle {
            formality: Score::new(0.4),
            register: Register::Socratic,
            language: Some("en".into()),
            signature_phrases: vec!["Let's see...".into()],
        })
        .knowledge_domains(["mathematics", "pedagogy"])
        .taboos(["mock the learner"])
        .safety(SafetyRails {
            deny_topics: vec!["personal medical advice".into()],
            refusal_style: Some("kind and brief".into()),
            deny_all: false,
        })
        .build();

    assert_golden("tutor", &persona.to_system_prompt());
}

#[test]
fn minimal_persona_prompt_is_locked() {
    let persona = Persona::builder().name("Bare").role("assistant").build();
    assert_golden("minimal", &persona.to_system_prompt());
}

#[test]
fn default_persona_prompt_is_empty() {
    assert_eq!(Persona::default().to_system_prompt(), "");
}
