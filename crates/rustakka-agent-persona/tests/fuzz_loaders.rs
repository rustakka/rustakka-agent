//! Fuzz-lite tests for the persona loaders.
//!
//! These are not proptest/cargo-fuzz — they're deterministic
//! byte-sequence sweeps that prove the loader returns `Err` rather
//! than panicking on malformed input. Covers JSON, YAML (when
//! enabled), and TOML (when enabled).

use rustakka_agent_persona::Persona;

fn seeds() -> Vec<Vec<u8>> {
    let mut v: Vec<Vec<u8>> = vec![
        Vec::new(),
        b"".to_vec(),
        b"{".to_vec(),
        b"}".to_vec(),
        b"[]".to_vec(),
        b"null".to_vec(),
        b"{\"identity\":".to_vec(),
        b"{\"identity\":{\"name\":null}}".to_vec(),
        b"\xff\xfe\xfd".to_vec(),
        b"{\"iq\":{\"planning_hops\":-1}}".to_vec(),
        b"{\"safety\":{\"deny_all\":true},\"taboos\":[\"x\"]}".to_vec(),
        b"identity:\n  name: null\n".to_vec(),
        b"identity = { name = \"Ada\" }".to_vec(),
    ];
    // Add a few random-ish corruptions of a valid JSON persona.
    let valid =
        br#"{"identity":{"name":"Ada"},"values":["a","b"],"iq":{"planning_hops":2}}"#.to_vec();
    for i in 0..valid.len() {
        let mut m = valid.clone();
        m[i] = 0x01;
        v.push(m);
    }
    v
}

#[test]
fn persona_from_json_never_panics() {
    for s in seeds() {
        if let Ok(text) = std::str::from_utf8(&s) {
            let _ = Persona::from_json(text);
        }
    }
}

#[cfg(feature = "yaml")]
#[test]
fn persona_from_yaml_never_panics() {
    for s in seeds() {
        if let Ok(text) = std::str::from_utf8(&s) {
            let _ = Persona::from_yaml(text);
        }
    }
}

#[cfg(feature = "toml")]
#[test]
fn persona_from_toml_never_panics() {
    for s in seeds() {
        if let Ok(text) = std::str::from_utf8(&s) {
            let _ = Persona::from_toml(text);
        }
    }
}
