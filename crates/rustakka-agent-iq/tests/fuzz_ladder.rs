//! Fuzz-lite tests for the IQ-ladder JSON loader. Proves that
//! `IqLadderSpec::from_json` never panics on malformed input.

use rustakka_agent_iq::ladder::IqLadderSpec;

fn seeds() -> Vec<&'static [u8]> {
    vec![
        b"",
        b"{",
        b"}",
        b"null",
        b"[]",
        b"{\"tiers\":[]}",
        b"{\"tiers\":{\"Unknown\":{}}}",
        b"{\"default_carryings\":{\"temperature\":\"not a number\"}}",
        b"{\"default_rung\":{\"name\":null}}",
        b"{\"tiers\":{\"Analyst\":{\"rungs\":[{\"name\":123}]}}}",
        b"\xff\xfe",
        b"{\"tiers\":{\"Analyst\":{\"rungs\":[{}]}}}",
    ]
}

#[test]
fn ladder_from_json_never_panics() {
    for s in seeds() {
        if let Ok(text) = std::str::from_utf8(s) {
            let _ = IqLadderSpec::from_json(text);
        }
    }
}

#[test]
fn ladder_from_json_valid_roundtrips() {
    let json = r#"{
        "default_carryings": { "temperature": 0.2 },
        "tiers": {
            "Analyst": {
                "rungs": [ { "name": "gpt-4o-mini" } ]
            }
        }
    }"#;
    let spec = IqLadderSpec::from_json(json).unwrap();
    assert_eq!(spec.default_carryings.temperature, Some(0.2));
}
