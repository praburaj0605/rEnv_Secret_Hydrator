//! Secret rotation / refresh configuration (FR-004).

use std::time::Duration;

/// Retry policy for provider fetches.
#[derive(Clone, Debug)]
pub struct RetryPolicy {
    /// Maximum attempts (including the first).
    pub max_attempts: u32,
    /// Base delay before first retry.
    pub base_delay: Duration,
    /// Maximum delay cap.
    pub max_delay: Duration,
    /// Whether to add jitter.
    pub jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(5),
            jitter: true,
        }
    }
}

impl RetryPolicy {
    /// Compute delay for attempt index (0-based after first failure).
    #[must_use]
    pub fn delay_for(&self, attempt: u32) -> Duration {
        let exp = self.base_delay.saturating_mul(2u32.saturating_pow(attempt));
        let mut delay = exp.min(self.max_delay);
        if self.jitter {
            let nanos = delay.as_nanos().min(u128::from(u64::MAX)) as u64;
            let jitter = rand::random::<u64>() % (nanos / 2 + 1);
            delay = Duration::from_nanos(nanos.saturating_sub(jitter) / 2 + jitter / 2);
        }
        delay
    }
}

/// Behavior when refresh fails.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FailurePolicy {
    /// Keep serving the last successfully loaded configuration.
    #[default]
    KeepLastGood,
    /// Surface the error to callers / watch channel.
    FailFast,
    /// Fall back to local providers / defaults on hard failure.
    FallbackLocal,
}

/// Background refresh settings.
#[derive(Clone, Debug)]
pub struct RefreshConfig {
    /// How often to refresh.
    pub interval: Duration,
    /// Retry policy.
    pub retry: RetryPolicy,
    /// Failure handling.
    pub on_failure: FailurePolicy,
}

impl Default for RefreshConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(300),
            retry: RetryPolicy::default(),
            on_failure: FailurePolicy::KeepLastGood,
        }
    }
}
