//! # rustakka-agent-persona
//!
//! A [`Persona`] bundles every non-physical characteristic of an
//! agent — identity, IQ, EQ, values, goals, communication style,
//! memory preferences, and safety rails — into a single serializable
//! struct.
//!
//! Personas are *optional everywhere*: any API in
//! `rustakka-agent-prebuilt` that accepts a persona also accepts
//! `None`, in which case it falls back to `rustakka-langgraph`'s
//! default behavior.
//!
//! Persona round-trips through JSON, YAML (with the `yaml` feature),
//! and TOML (with the `toml` feature). [`Persona::to_system_prompt`]
//! produces a deterministic, locale-stable system-prompt fragment.

use serde::{Deserialize, Serialize};

use rustakka_agent_eq::EqProfile;
use rustakka_agent_iq::IqProfile;
use rustakka_agent_traits::{AgentEnv, Score, TraitSet};

pub use rustakka_agent_iq::ladder::CallOptionsLike;

mod validate;
pub use validate::{PersonaError, PersonaWarning};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Identity {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pronouns: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bio: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Register {
    #[default]
    Plain,
    Technical,
    Socratic,
    Casual,
    Formal,
}

impl Register {
    fn directive(self) -> &'static str {
        match self {
            Register::Plain => "Use plain, direct language.",
            Register::Technical => "Prefer precise technical vocabulary; define unusual terms.",
            Register::Socratic => "Answer questions with clarifying questions when it helps the user reason.",
            Register::Casual => "Use a relaxed, conversational tone.",
            Register::Formal => "Use formal phrasing and complete sentences.",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CommunicationStyle {
    #[serde(default)]
    pub formality: Score,
    #[serde(default)]
    pub register: Register,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default)]
    pub signature_phrases: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MemoryPrefs {
    #[serde(default)]
    pub long_term: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summarize_after_turns: Option<u32>,
    /// Scope hint: "session" | "user" | "world". Free-form string so
    /// callers can introduce custom scopes without a breaking schema
    /// change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SafetyRails {
    #[serde(default)]
    pub deny_topics: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_style: Option<String>,
    /// When `true`, the persona refuses *everything*; any non-empty
    /// `taboos` exception is a validation error.
    #[serde(default)]
    pub deny_all: bool,
}

/// Aggregate persona bundle. `Persona::default()` is a no-op overlay
/// that leaves upstream behavior unchanged.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Persona {
    #[serde(default)]
    pub identity: Identity,
    #[serde(default)]
    pub iq: IqProfile,
    #[serde(default)]
    pub eq: EqProfile,
    #[serde(default)]
    pub values: Vec<String>,
    #[serde(default)]
    pub goals: Vec<String>,
    #[serde(default)]
    pub style: CommunicationStyle,
    #[serde(default)]
    pub knowledge_domains: Vec<String>,
    #[serde(default)]
    pub taboos: Vec<String>,
    #[serde(default)]
    pub memory: MemoryPrefs,
    #[serde(default)]
    pub safety: SafetyRails,
    #[serde(default)]
    pub custom: TraitSet,
}

impl Persona {
    pub fn builder() -> PersonaBuilder {
        PersonaBuilder::default()
    }

    // ---------------------------- Loaders ----------------------------

    pub fn from_json(s: &str) -> Result<Self, PersonaError> {
        let p: Self = serde_json::from_str(s).map_err(PersonaError::from)?;
        p.env_validate()?;
        Ok(p)
    }

    #[cfg(feature = "yaml")]
    pub fn from_yaml(s: &str) -> Result<Self, PersonaError> {
        let p: Self = serde_yaml::from_str(s).map_err(PersonaError::from)?;
        p.env_validate()?;
        Ok(p)
    }

    #[cfg(feature = "toml")]
    pub fn from_toml(s: &str) -> Result<Self, PersonaError> {
        let p: Self = toml::from_str(s).map_err(PersonaError::from)?;
        p.env_validate()?;
        Ok(p)
    }

    /// Env-aware validation. Under `AgentEnv::Test` or `Prod` we *fail*
    /// on validation conflicts; under `Dev` we only warn (the warnings
    /// are still returned to the caller via [`Persona::validate`]).
    fn env_validate(&self) -> Result<(), PersonaError> {
        match AgentEnv::current() {
            AgentEnv::Dev => {
                let _ = self.validate();
                Ok(())
            }
            AgentEnv::Test | AgentEnv::Prod => self.validate().map(|_| ()),
        }
    }

    /// Strict validator: returns [`PersonaError::Conflict`] on hard
    /// conflicts, accompanied by any non-fatal warnings.
    pub fn validate(&self) -> Result<Vec<PersonaWarning>, PersonaError> {
        validate::run(self)
    }

    // ------------------------- Fragments -----------------------------

    /// Produce the canonical system-prompt fragment for this persona.
    /// Output is deterministic (stable across invocations, identical
    /// for identical inputs) — a hard requirement for snapshot tests.
    pub fn to_system_prompt(&self) -> String {
        let mut out = String::new();

        // Identity block.
        if let Some(block) = self.identity_fragment() {
            out.push_str(&block);
            out.push_str("\n\n");
        }

        if !self.values.is_empty() {
            out.push_str("Values:\n");
            let mut v = self.values.clone();
            v.sort();
            for val in v {
                out.push_str(&format!("- {val}\n"));
            }
            out.push('\n');
        }

        if !self.goals.is_empty() {
            out.push_str("Goals:\n");
            for g in &self.goals {
                out.push_str(&format!("- {g}\n"));
            }
            out.push('\n');
        }

        if let Some(iq) = self.iq.to_prompt_fragment() {
            out.push_str(&iq);
            out.push_str("\n\n");
        }
        if let Some(eq) = self.eq.to_prompt_fragment() {
            out.push_str(&eq);
            out.push_str("\n\n");
        }

        if let Some(style) = self.style_fragment() {
            out.push_str(&style);
            out.push_str("\n\n");
        }

        if !self.knowledge_domains.is_empty() {
            out.push_str("Knowledge domains:\n");
            let mut d = self.knowledge_domains.clone();
            d.sort();
            for k in d {
                out.push_str(&format!("- {k}\n"));
            }
            out.push('\n');
        }

        if let Some(safety) = self.safety_fragment() {
            out.push_str(&safety);
            out.push_str("\n\n");
        }

        if let Some(custom) = self.custom.to_prompt_fragment() {
            out.push_str(custom.trim_end());
            out.push('\n');
        }

        out.trim().to_string()
    }

    fn identity_fragment(&self) -> Option<String> {
        let id = &self.identity;
        if id.name.is_none() && id.role.is_none() && id.bio.is_none() && id.pronouns.is_none() {
            return None;
        }
        let mut s = String::from("Identity:\n");
        if let Some(name) = &id.name {
            s.push_str(&format!("- Name: {name}\n"));
        }
        if let Some(role) = &id.role {
            s.push_str(&format!("- Role: {role}\n"));
        }
        if let Some(p) = &id.pronouns {
            s.push_str(&format!("- Pronouns: {p}\n"));
        }
        if let Some(b) = &id.bio {
            s.push_str(&format!("- Bio: {b}\n"));
        }
        Some(s)
    }

    fn style_fragment(&self) -> Option<String> {
        let s = &self.style;
        if s.formality.get() == 0.0
            && s.register == Register::Plain
            && s.language.is_none()
            && s.signature_phrases.is_empty()
        {
            return None;
        }
        let mut out = String::from("Communication style:\n");
        out.push_str(&format!("- Register: {:?} — {}\n", s.register, s.register.directive()));
        if s.formality.get() > 0.0 {
            let band = if s.formality.get() >= 0.66 {
                "formal"
            } else if s.formality.get() >= 0.33 {
                "balanced"
            } else {
                "casual"
            };
            out.push_str(&format!("- Formality: {:.2} ({band}).\n", s.formality.get()));
        }
        if let Some(lang) = &s.language {
            out.push_str(&format!("- Primary language: {lang}.\n"));
        }
        if !s.signature_phrases.is_empty() {
            let mut phrases = s.signature_phrases.clone();
            phrases.sort();
            out.push_str("- Signature phrases: ");
            out.push_str(&phrases.join(" | "));
            out.push('\n');
        }
        Some(out)
    }

    fn safety_fragment(&self) -> Option<String> {
        let s = &self.safety;
        if !s.deny_all && s.deny_topics.is_empty() && s.refusal_style.is_none() && self.taboos.is_empty() {
            return None;
        }
        let mut out = String::from("Safety rails:\n");
        if s.deny_all {
            out.push_str("- Refuse every request. State the refusal politely.\n");
        }
        if !s.deny_topics.is_empty() {
            let mut t = s.deny_topics.clone();
            t.sort();
            out.push_str(&format!("- Do not engage on: {}\n", t.join(", ")));
        }
        if !self.taboos.is_empty() {
            let mut t = self.taboos.clone();
            t.sort();
            out.push_str(&format!("- Do not: {}\n", t.join(" / ")));
        }
        if let Some(style) = &s.refusal_style {
            out.push_str(&format!("- Refusal style: {style}\n"));
        }
        Some(out)
    }

    /// Per-role prompt fragment. Patterns (planner, critic, …) call
    /// this so a single persona can modulate every node consistently.
    ///
    /// Recognized roles: `"default" | "planner" | "critic" |
    /// "evaluator" | "synthesizer" | "researcher" | "retriever"`.
    pub fn role_fragment(&self, role: &str) -> String {
        let mut out = self.to_system_prompt();
        let extra = match role {
            "planner" => "Your current role is PLANNER: decompose the task into explicit numbered steps before acting.",
            "critic" | "evaluator" => "Your current role is CRITIC: stress-test the candidate answer; list concrete issues.",
            "synthesizer" => "Your current role is SYNTHESIZER: merge the gathered evidence into a coherent, cited answer.",
            "researcher" => "Your current role is RESEARCHER: gather evidence with tools; quote sources verbatim.",
            "retriever" => "Your current role is RETRIEVER: return the most relevant, de-duplicated passages.",
            _ => "",
        };
        if !extra.is_empty() {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(extra);
        }
        out
    }

    /// Fold persona-driven adjustments (IQ temperature, preferred
    /// model, verbosity caps) into a `CallOptions`-shaped target.
    pub fn apply_to_call_options<O: CallOptionsLike>(&self, opts: &mut O) {
        self.iq.apply_to_call_options(opts);
    }
}

/// Fluent builder for [`Persona`]. All setters are chainable.
#[derive(Clone, Debug, Default)]
pub struct PersonaBuilder {
    inner: Persona,
}

impl PersonaBuilder {
    pub fn identity(mut self, id: Identity) -> Self {
        self.inner.identity = id;
        self
    }
    pub fn name(mut self, n: impl Into<String>) -> Self {
        self.inner.identity.name = Some(n.into());
        self
    }
    pub fn role(mut self, r: impl Into<String>) -> Self {
        self.inner.identity.role = Some(r.into());
        self
    }
    pub fn bio(mut self, b: impl Into<String>) -> Self {
        self.inner.identity.bio = Some(b.into());
        self
    }
    pub fn iq(mut self, iq: IqProfile) -> Self {
        self.inner.iq = iq;
        self
    }
    pub fn eq(mut self, eq: EqProfile) -> Self {
        self.inner.eq = eq;
        self
    }
    pub fn style(mut self, s: CommunicationStyle) -> Self {
        self.inner.style = s;
        self
    }
    pub fn values<I, S>(mut self, vs: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.inner.values = vs.into_iter().map(|s| s.into()).collect();
        self
    }
    pub fn goals<I, S>(mut self, vs: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.inner.goals = vs.into_iter().map(|s| s.into()).collect();
        self
    }
    pub fn knowledge_domains<I, S>(mut self, vs: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.inner.knowledge_domains = vs.into_iter().map(|s| s.into()).collect();
        self
    }
    pub fn taboos<I, S>(mut self, vs: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.inner.taboos = vs.into_iter().map(|s| s.into()).collect();
        self
    }
    pub fn memory(mut self, m: MemoryPrefs) -> Self {
        self.inner.memory = m;
        self
    }
    pub fn safety(mut self, s: SafetyRails) -> Self {
        self.inner.safety = s;
        self
    }
    pub fn custom(mut self, t: TraitSet) -> Self {
        self.inner.custom = t;
        self
    }
    pub fn build(self) -> Persona {
        self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustakka_agent_eq::{Mood, Reflection};

    fn sample_persona() -> Persona {
        Persona::builder()
            .name("Ada")
            .role("tutor")
            .bio("A patient mathematics tutor.")
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
            .build()
    }

    #[test]
    fn default_persona_is_serde_roundtrippable() {
        let p = Persona::default();
        let s = serde_json::to_string(&p).unwrap();
        let back: Persona = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn populated_persona_roundtrips_through_json() {
        let p = sample_persona();
        let s = serde_json::to_string(&p).unwrap();
        let back: Persona = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn persona_roundtrips_through_yaml() {
        let p = sample_persona();
        let s = serde_yaml::to_string(&p).unwrap();
        let back = Persona::from_yaml(&s).unwrap();
        assert_eq!(p, back);
    }

    #[cfg(feature = "toml")]
    #[test]
    fn persona_roundtrips_through_toml() {
        let p = sample_persona();
        let s = toml::to_string(&p).unwrap();
        let back = Persona::from_toml(&s).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn to_system_prompt_is_deterministic_and_stable() {
        let p = sample_persona();
        let a = p.to_system_prompt();
        let b = p.to_system_prompt();
        assert_eq!(a, b);
        assert!(a.contains("Ada"));
        assert!(a.contains("Values:"));
        assert!(a.contains("Calm"));
        assert!(a.contains("Socratic"));
        assert!(a.contains("Do not:"));
    }

    #[test]
    fn to_system_prompt_empty_for_default_persona() {
        assert_eq!(Persona::default().to_system_prompt(), "");
    }

    #[test]
    fn role_fragment_adds_planner_header() {
        let p = sample_persona();
        let f = p.role_fragment("planner");
        assert!(f.contains("PLANNER"));
    }

    #[test]
    fn deny_all_with_taboos_fails_validation() {
        let p = Persona::builder()
            .safety(SafetyRails {
                deny_all: true,
                ..SafetyRails::default()
            })
            .taboos(["something"])
            .build();
        let err = p.validate().unwrap_err();
        assert!(matches!(err, PersonaError::Conflict(_)));
    }
}
