//! Configuration value types used during resolution.

use crate::secret::SecretString;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;

/// Ordered map of configuration keys to values.
pub type ConfigMap = BTreeMap<String, ConfigValue>;

/// A resolved configuration value.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConfigValue {
    /// Plain string (non-secret).
    String(String),
    /// Secret-bearing string.
    Secret(SecretString),
    /// Nested object.
    Object(ConfigMap),
    /// JSON-compatible scalar / array for typed deserialize flexibility.
    Json(JsonValue),
}

impl ConfigValue {
    /// Create a secret value.
    pub fn secret(value: impl Into<String>) -> Self {
        Self::Secret(SecretString::new(value))
    }

    /// Create a plain string value.
    pub fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }

    /// Expose as string if possible (secrets included).
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            Self::Secret(s) => Some(s.expose()),
            Self::Json(JsonValue::String(s)) => Some(s),
            _ => None,
        }
    }

    /// Convert into a JSON value suitable for serde deserialization into `T`.
    #[must_use]
    pub fn into_json(self) -> JsonValue {
        match self {
            Self::String(s) => JsonValue::String(s),
            Self::Secret(s) => JsonValue::String(s.expose().to_string()),
            Self::Object(map) => {
                let mut obj = serde_json::Map::new();
                for (k, v) in map {
                    obj.insert(k, v.into_json());
                }
                JsonValue::Object(obj)
            }
            Self::Json(v) => v,
        }
    }
}

/// Reference describing how to fetch a secret from a provider.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecretRef {
    /// Logical configuration key.
    pub key: String,
    /// Optional provider-specific path / ARN / name.
    #[serde(default)]
    pub path: Option<String>,
    /// Optional provider hint.
    #[serde(default)]
    pub provider: Option<String>,
}

/// Flatten a `ConfigMap` into a JSON object for typed loading.
#[must_use]
pub fn map_to_json(map: ConfigMap) -> JsonValue {
    let mut obj = serde_json::Map::new();
    for (k, v) in map {
        obj.insert(k, v.into_json());
    }
    JsonValue::Object(obj)
}

/// Merge `overlay` into `base` (overlay wins on key conflict).
pub fn merge_maps(base: &mut ConfigMap, overlay: ConfigMap) {
    for (k, v) in overlay {
        match (base.get_mut(&k), v) {
            (Some(ConfigValue::Object(dst)), ConfigValue::Object(src)) => {
                merge_maps(dst, src);
            }
            (_, other) => {
                base.insert(k, other);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_nested_overlay_wins() {
        let mut base = ConfigMap::new();
        let mut inner = ConfigMap::new();
        inner.insert("a".into(), ConfigValue::string("1"));
        base.insert("obj".into(), ConfigValue::Object(inner));

        let mut overlay_inner = ConfigMap::new();
        overlay_inner.insert("a".into(), ConfigValue::string("2"));
        overlay_inner.insert("b".into(), ConfigValue::string("3"));
        let mut overlay = ConfigMap::new();
        overlay.insert("obj".into(), ConfigValue::Object(overlay_inner));

        merge_maps(&mut base, overlay);
        match &base["obj"] {
            ConfigValue::Object(o) => {
                assert_eq!(o["a"].as_str(), Some("2"));
                assert_eq!(o["b"].as_str(), Some("3"));
            }
            _ => panic!("expected object"),
        }
    }

    #[test]
    fn map_to_json_flattens_secrets() {
        let mut map = ConfigMap::new();
        map.insert("database_url".into(), ConfigValue::secret("s"));
        let json = map_to_json(map);
        assert_eq!(json["database_url"], "s");
    }
}
