//! Property tests for SecretString redaction (FR-009).

use esh_core::SecretString;
use proptest::prelude::*;

proptest! {
    #[test]
    fn secret_string_always_fully_redacted(s in "\\PC*") {
        prop_assume!(!s.is_empty());
        prop_assume!(s.len() <= 128);
        let secret = SecretString::new(s.clone());
        prop_assert_eq!(format!("{secret:?}"), "SecretString(***REDACTED***)");
        prop_assert_eq!(format!("{secret}"), "***REDACTED***");
        prop_assert_eq!(secret.expose(), s.as_str());
    }
}
