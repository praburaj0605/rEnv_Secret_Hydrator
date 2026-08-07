//! Configuration validation (FR-008).

use serde_json::Value as JsonValue;
use std::fmt;
use thiserror::Error;

/// Field-level validation failure (key names only — never values).
#[derive(Debug, Error, Clone)]
#[error("validation errors: {summary}")]
pub struct ValidationError {
    /// Human summary.
    summary: String,
    /// Per-field messages.
    pub fields: Vec<(String, String)>,
}

impl ValidationError {
    /// Create from field errors.
    #[must_use]
    pub fn from_fields(fields: Vec<(String, String)>) -> Self {
        let summary = fields
            .iter()
            .map(|(k, m)| format!("{k}: {m}"))
            .collect::<Vec<_>>()
            .join("; ");
        Self { summary, fields }
    }
}

/// Validation rule for a key.
#[derive(Clone, Debug)]
pub enum FieldRule {
    /// Key must be present and non-empty.
    Required,
    /// Value must match regex (as string).
    Regex(String),
    /// Custom predicate name for docs; actual check via closure in Validator builder is limited —
    /// use `Validator::check` for custom.
    NonEmpty,
}

/// Validates a JSON configuration object before typed deserialize completes.
#[derive(Clone, Default)]
pub struct Validator {
    rules: Vec<(String, FieldRule)>,
}

impl Validator {
    /// Create an empty validator.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Require a key.
    #[must_use]
    pub fn require(mut self, key: impl Into<String>) -> Self {
        self.rules.push((key.into(), FieldRule::Required));
        self
    }

    /// Require non-empty string.
    #[must_use]
    pub fn non_empty(mut self, key: impl Into<String>) -> Self {
        self.rules.push((key.into(), FieldRule::NonEmpty));
        self
    }

    /// Require string matching a simple glob (`prefix*`, `*suffix`, or exact).
    #[must_use]
    pub fn pattern(mut self, key: impl Into<String>, pattern: impl Into<String>) -> Self {
        self.rules.push((key.into(), FieldRule::Regex(pattern.into())));
        self
    }

    /// Validate a JSON object. Does not echo values into errors.
    pub fn validate_json(&self, value: &JsonValue) -> Result<(), ValidationError> {
        let obj = value.as_object();
        let mut errors = Vec::new();

        for (key, rule) in &self.rules {
            let field = obj.and_then(|o| o.get(key));
            match rule {
                FieldRule::Required => {
                    if field.is_none() || field == Some(&JsonValue::Null) {
                        errors.push((key.clone(), "missing required field".into()));
                    }
                }
                FieldRule::NonEmpty => match field {
                    Some(JsonValue::String(s)) if s.is_empty() => {
                        errors.push((key.clone(), "must be non-empty".into()));
                    }
                    Some(JsonValue::String(_)) => {}
                    Some(_) => {}
                    None => errors.push((key.clone(), "missing required field".into())),
                },
                FieldRule::Regex(pattern) => {
                    // Minimal check without pulling regex crate into core by default:
                    // treat as "contains" sentinel for now — providers may wrap denser checks.
                    if let Some(JsonValue::String(s)) = field {
                        if !simple_glob_match(pattern, s) {
                            errors.push((key.clone(), "invalid format".into()));
                        }
                    } else {
                        errors.push((key.clone(), "missing or not a string".into()));
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ValidationError::from_fields(errors))
        }
    }
}

impl fmt::Debug for Validator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Validator")
            .field("rules", &self.rules.len())
            .finish()
    }
}

fn simple_glob_match(pattern: &str, value: &str) -> bool {
    // Extremely small matcher: exact, prefix*, or *suffix.
    if let Some(rest) = pattern.strip_suffix('*') {
        return value.starts_with(rest);
    }
    if let Some(rest) = pattern.strip_prefix('*') {
        return value.ends_with(rest);
    }
    pattern == value
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn required_field_missing() {
        let v = Validator::new().require("database_url");
        let err = v.validate_json(&json!({"region": "us"})).unwrap_err();
        assert!(err.to_string().contains("database_url"));
        assert!(!err.to_string().contains("password"));
    }

    #[test]
    fn non_empty_rejects_blank() {
        let v = Validator::new().non_empty("token");
        let err = v.validate_json(&json!({"token": ""})).unwrap_err();
        assert!(err.to_string().contains("token"));
        assert!(err.to_string().contains("non-empty"));
    }

    #[test]
    fn glob_prefix_pattern() {
        let v = Validator::new().pattern("url", "postgres*");
        assert!(v
            .validate_json(&json!({"url": "postgres://x"}))
            .is_ok());
        assert!(v.validate_json(&json!({"url": "mysql://x"})).is_err());
    }

    #[test]
    fn multi_field_errors_omit_values() {
        let v = Validator::new().require("a").require("b");
        let err = v
            .validate_json(&json!({"a": "secret-value-xyz"}))
            .unwrap_err();
        assert!(err.to_string().contains("b"));
        assert!(!err.to_string().contains("secret-value-xyz"));
    }
}
