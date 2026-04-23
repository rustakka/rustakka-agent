//! Persona validation.
//!
//! Splits concerns into hard [`PersonaError`]s (non-recoverable) and
//! soft [`PersonaWarning`]s. [`Persona::env_validate`] decides whether
//! warnings upgrade to errors based on [`AgentEnv`].

use super::Persona;

/// Non-recoverable persona conflicts.
#[derive(Debug, thiserror::Error)]
pub enum PersonaError {
    #[error("persona conflict: {0}")]
    Conflict(String),
    #[error("invalid persona JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[cfg(feature = "yaml")]
    #[error("invalid persona YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[cfg(feature = "toml")]
    #[error("invalid persona TOML: {0}")]
    TomlDe(#[from] toml::de::Error),
}

/// Non-fatal issues a persona can carry (returned from
/// [`crate::Persona::validate`] alongside a successful result).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersonaWarning {
    EmptyIdentity,
    ContradictoryValuesAndGoals,
    ExcessiveReflection,
}

pub(crate) fn run(p: &Persona) -> Result<Vec<PersonaWarning>, PersonaError> {
    // Hard conflicts --------------------------------------------------
    if p.safety.deny_all && !p.taboos.is_empty() {
        return Err(PersonaError::Conflict(
            "safety.deny_all = true is incompatible with any `taboos` exception".into(),
        ));
    }
    if p.iq.temperature.map(|t| !t.is_finite() || !(0.0..=2.0).contains(&t)).unwrap_or(false) {
        return Err(PersonaError::Conflict(format!(
            "iq.temperature must be in [0.0, 2.0]; got {:?}",
            p.iq.temperature
        )));
    }

    // Soft warnings ---------------------------------------------------
    let mut warnings = Vec::new();
    if p.identity.name.is_none()
        && p.identity.role.is_none()
        && p.identity.bio.is_none()
    {
        warnings.push(PersonaWarning::EmptyIdentity);
    }
    if !p.values.is_empty()
        && !p.goals.is_empty()
        && p.values.iter().any(|v| p.taboos.contains(v))
    {
        warnings.push(PersonaWarning::ContradictoryValuesAndGoals);
    }
    if p.eq.reflection_cadence == rustakka_agent_eq::Reflection::AfterEachTurn
        && p.iq.planning_hops > 8
    {
        warnings.push(PersonaWarning::ExcessiveReflection);
    }
    Ok(warnings)
}
