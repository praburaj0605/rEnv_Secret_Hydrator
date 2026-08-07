//! Secret-bearing types that never print plaintext.

use serde::de::{Deserialize, Deserializer, Visitor};
use serde::Serialize;
use std::fmt;
use zeroize::{Zeroize, ZeroizeOnDrop};

const REDACTED: &str = "***REDACTED***";

/// UTF-8 secret string. `Debug`/`Display` always redact the value.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecretString(String);

impl SecretString {
    /// Wrap a plaintext secret.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrow the plaintext. Prefer short-lived use only.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Consume and return the inner string (caller becomes responsible for zeroization).
    #[must_use]
    pub fn into_inner(mut self) -> String {
        let out = std::mem::take(&mut self.0);
        // Drop still zeroizes the emptied buffer.
        out
    }

    /// Length in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the secret is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretString(")?;
        f.write_str(REDACTED)?;
        f.write_str(")")
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTED)
    }
}

impl From<String> for SecretString {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for SecretString {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl AsRef<str> for SecretString {
    fn as_ref(&self) -> &str {
        self.expose()
    }
}

impl Serialize for SecretString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Serialization of secrets is intentional only for trusted sinks.
        serializer.serialize_str(self.expose())
    }
}

impl<'de> Deserialize<'de> for SecretString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;
        impl Visitor<'_> for V {
            type Value = SecretString;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a string secret")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(SecretString::new(v))
            }

            fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Self::Value, E> {
                Ok(SecretString::new(v))
            }
        }
        deserializer.deserialize_string(V)
    }
}

/// Opaque secret bytes. `Debug`/`Display` always redact the value.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecretBytes(Vec<u8>);

impl SecretBytes {
    /// Wrap plaintext bytes.
    pub fn new(value: impl Into<Vec<u8>>) -> Self {
        Self(value.into())
    }

    /// Borrow plaintext bytes.
    #[must_use]
    pub fn expose(&self) -> &[u8] {
        &self.0
    }

    /// Length in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for SecretBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretBytes(")?;
        f.write_str(REDACTED)?;
        f.write_str(")")
    }
}

impl fmt::Display for SecretBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTED)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_string_never_leaks_in_debug_or_display() {
        let secret = SecretString::new("super-secret-password-xyz");
        let debug = format!("{secret:?}");
        let display = format!("{secret}");
        assert!(!debug.contains("super-secret"));
        assert!(!display.contains("super-secret"));
        assert!(debug.contains(REDACTED));
        assert_eq!(display, REDACTED);
        assert_eq!(secret.expose(), "super-secret-password-xyz");
    }

    #[test]
    fn secret_bytes_never_leaks() {
        let secret = SecretBytes::new(b"token-abc".to_vec());
        assert!(!format!("{secret:?}").contains("token-abc"));
        assert_eq!(format!("{secret}"), REDACTED);
    }
}
