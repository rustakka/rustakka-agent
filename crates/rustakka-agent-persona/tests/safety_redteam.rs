//! Red-team suite (Phase 9) — sanity checks that safety-rail
//! invariants hold across common prompt-shape variations. These are
//! asserting *prompt-contents* guarantees, not jailbreak resistance of
//! the underlying model.

use rustakka_agent_persona::{Persona, PersonaError, SafetyRails};

#[test]
fn deny_all_and_taboos_is_rejected() {
    let p = Persona::builder()
        .safety(SafetyRails {
            deny_all: true,
            ..SafetyRails::default()
        })
        .taboos(["anything"])
        .build();
    assert!(matches!(p.validate().unwrap_err(), PersonaError::Conflict(_)));
}

#[test]
fn deny_topics_are_visible_in_system_prompt() {
    let p = Persona::builder()
        .safety(SafetyRails {
            deny_topics: vec!["medical diagnoses".into(), "legal advice".into()],
            ..SafetyRails::default()
        })
        .build();
    let s = p.to_system_prompt();
    assert!(s.contains("medical diagnoses"));
    assert!(s.contains("legal advice"));
}

#[test]
fn refusal_style_appears_when_set() {
    let p = Persona::builder()
        .safety(SafetyRails {
            deny_topics: vec!["x".into()],
            refusal_style: Some("kindly decline".into()),
            ..SafetyRails::default()
        })
        .build();
    assert!(p.to_system_prompt().contains("kindly decline"));
}

#[test]
fn taboos_appear_in_deterministic_order() {
    let p = Persona::builder()
        .taboos(["zeta", "alpha", "mu"])
        .build();
    let s = p.to_system_prompt();
    let alpha = s.find("alpha").unwrap();
    let mu = s.find("mu").unwrap();
    let zeta = s.find("zeta").unwrap();
    assert!(alpha < mu && mu < zeta);
}

#[test]
fn deny_all_without_taboos_is_accepted_and_visible() {
    let p = Persona::builder()
        .safety(SafetyRails {
            deny_all: true,
            ..SafetyRails::default()
        })
        .build();
    assert!(p.validate().is_ok());
    assert!(p.to_system_prompt().contains("Refuse every request"));
}
