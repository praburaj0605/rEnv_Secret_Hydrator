//! Canonical fixture keys and sample secret values for tests.

/// Logical database URL config key.
pub const DATABASE_URL: &str = "database_url";

/// Logical API token config key.
pub const API_TOKEN: &str = "api_token";

/// Sample secret value (never expect this in Debug/Display/audit).
pub const SAMPLE_SECRET: &str = "postgres://user:s3cr3t-needle-xyz@localhost/app";

/// Sample token value.
pub const SAMPLE_TOKEN: &str = "tok_test_needle_abc123";
