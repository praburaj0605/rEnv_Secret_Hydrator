//! Leak-detection helpers (FR-009).

use std::fmt::{Debug, Display};

/// A plaintext needle that must never appear in rendered output.
#[derive(Clone, Debug)]
pub struct LeakNeedle(pub String);

impl LeakNeedle {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

/// Assert that `Debug` and `Display` of `value` do not contain `needle`.
pub fn assert_no_leak<T: Debug + Display>(value: &T, needle: &str) {
    let debug = format!("{value:?}");
    let display = format!("{value}");
    assert!(
        !debug.contains(needle),
        "Debug leaked secret needle `{needle}`: {debug}"
    );
    assert!(
        !display.contains(needle),
        "Display leaked secret needle `{needle}`: {display}"
    );
}

/// Assert that an arbitrary string does not contain the needle.
pub fn assert_no_leak_in(haystack: &str, needle: &str) {
    assert!(
        !haystack.contains(needle),
        "haystack leaked secret needle `{needle}`: {haystack}"
    );
}
