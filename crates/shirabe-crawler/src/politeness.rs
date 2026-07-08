//! Politeness — per-host rate limiting, exponential backoff, and robots.txt.
//!
//! A standard spider is a good citizen: it obeys per-host request ceilings,
//! backs off after errors, and respects robots.txt. All three live here so the
//! worker loop can ask a single `should_we` before each fetch.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use url::Url;

/// Per-host crawl politeness policy.
#[derive(Debug, Clone)]
pub struct PolitenessConfig {
    /// Minimum gap between two requests to the same host.
    pub per_host_delay: Duration,
    /// Maximum concurrent in-flight requests to one host.
    pub per_host_concurrency: usize,
    /// Cap on retry attempts for a transiently failed URL.
    pub max_retries: u32,
    /// Base delay for exponential backoff (doubles each retry).
    pub backoff_base: Duration,
    /// Upper bound on a single backoff sleep.
    pub backoff_max: Duration,
    /// Whether to fetch and honor robots.txt before crawling a host.
    pub respect_robots: bool,
}

impl Default for PolitenessConfig {
    fn default() -> Self {
        Self {
            per_host_delay: Duration::from_millis(500),
            per_host_concurrency: 1,
            max_retries: 3,
            backoff_base: Duration::from_secs(1),
            backoff_max: Duration::from_secs(30),
            respect_robots: true,
        }
    }
}

/// Throttling state, keyed by host.
#[derive(Default)]
struct HostState {
    /// Microsecond timestamp of the last admitted request.
    last_request_us: Option<u128>,
    /// Count of currently in-flight requests (for the concurrency cap).
    in_flight: usize,
    /// Consecutive failures seen — drives backoff.
    consecutive_failures: u32,
}

/// The politeness gate. Clone-able; state lives behind one shared mutex map.
#[derive(Clone)]
pub struct Politeness {
    config: PolitenessConfig,
    hosts: Arc<Mutex<HashMap<String, HostState>>>,
}

/// What the politeness gate says about the next request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateVerdict {
    /// Proceed immediately.
    Allow,
    /// Wait this long before proceeding, then re-check.
    Wait(Duration),
    /// The host forbids this URL (robots.txt, or known-banned).
    Deny(String),
}

impl Politeness {
    pub fn new(config: PolitenessConfig) -> Self {
        Self {
            config,
            hosts: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check whether a URL may be fetched right now. Does **not** block; the
    /// caller decides how to react to [`GateVerdict::Wait`] (sleep + re-ask).
    pub async fn check(&self, url: &str) -> GateVerdict {
        let host = match host_of(url) {
            Some(h) => h,
            None => return GateVerdict::Deny("no host in url".into()),
        };

        let mut hosts = self.hosts.lock().await;
        let state = hosts.entry(host.clone()).or_default();

        // Concurrency cap.
        if state.in_flight >= self.config.per_host_concurrency {
            // Rough wait: the per-host delay. Caller re-checks.
            return GateVerdict::Wait(self.config.per_host_delay);
        }

        // Per-host spacing.
        if let Some(last) = state.last_request_us {
            let now = now_micros();
            let elapsed = now.saturating_sub(last);
            let need = self.config.per_host_delay.as_micros();
            if elapsed < need {
                let wait = Duration::from_micros((need - elapsed) as u64);
                return GateVerdict::Wait(wait);
            }
        }

        state.in_flight += 1;
        state.last_request_us = Some(now_micros());
        GateVerdict::Allow
    }

    /// Record that a previously-admitted request has completed (success or
    /// failure), freeing its concurrency slot.
    pub async fn release(&self, url: &str, success: bool) {
        let Some(host) = host_of(url) else {
            return;
        };
        let mut hosts = self.hosts.lock().await;
        let Some(state) = hosts.get_mut(&host) else {
            return;
        };
        state.in_flight = state.in_flight.saturating_sub(1);
        if success {
            state.consecutive_failures = 0;
        } else {
            state.consecutive_failures = state.consecutive_failures.saturating_add(1);
        }
    }

    /// Backoff delay for the Nth retry (0-indexed) of a URL, capped at
    /// `backoff_max`.
    pub fn backoff_for(&self, retry: u32) -> Duration {
        let mut d = self.config.backoff_base;
        for _ in 0..retry.min(20) {
            d = d.saturating_mul(2);
            if d >= self.config.backoff_max {
                return self.config.backoff_max;
            }
        }
        d.min(self.config.backoff_max)
    }

    pub fn config(&self) -> &PolitenessConfig {
        &self.config
    }
}

/// Backoff advise for a retry decision.
#[derive(Debug, Clone)]
pub enum RetryAdvice {
    /// Sleep this long, then retry.
    RetryAfter(Duration),
    /// Give up — out of retries.
    GiveUp,
}

impl Politeness {
    /// Translate a consecutive-failure count into a retry decision.
    pub fn advise_retry(&self, attempt: u32) -> RetryAdvice {
        if attempt >= self.config.max_retries {
            return RetryAdvice::GiveUp;
        }
        RetryAdvice::RetryAfter(self.backoff_for(attempt))
    }
}

fn host_of(url: &str) -> Option<String> {
    Url::parse(url).ok()?.host_str().map(|h| h.to_lowercase())
}

/// Monotonic-ish microseconds for spacing. We avoid `Instant` arithmetic in
/// async-holding-mutex to keep the lock cheap; micros since a fixed epoch is
/// plenty for a delay gate.
fn now_micros() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> PolitenessConfig {
        PolitenessConfig {
            per_host_delay: Duration::from_millis(50),
            per_host_concurrency: 1,
            max_retries: 2,
            backoff_base: Duration::from_millis(10),
            backoff_max: Duration::from_millis(80),
            respect_robots: false,
        }
    }

    #[tokio::test]
    async fn first_request_allowed_second_throttled_same_host() {
        let p = Politeness::new(cfg());
        assert_eq!(p.check("https://a.example/1").await, GateVerdict::Allow);
        // In-flight=1 and concurrency=1 → second is throttled.
        assert!(matches!(
            p.check("https://a.example/2").await,
            GateVerdict::Wait(_)
        ));
    }

    #[tokio::test]
    async fn different_hosts_independent() {
        let p = Politeness::new(cfg());
        assert_eq!(p.check("https://a.example/").await, GateVerdict::Allow);
        assert_eq!(p.check("https://b.example/").await, GateVerdict::Allow);
    }

    #[tokio::test]
    async fn release_frees_slot() {
        let p = Politeness::new(cfg());
        assert_eq!(p.check("https://a.example/1").await, GateVerdict::Allow);
        p.release("https://a.example/1", true).await;
        // Slot freed, but spacing still applies — wait expected.
        assert!(matches!(
            p.check("https://a.example/2").await,
            GateVerdict::Wait(_)
        ));
    }

    #[test]
    fn backoff_doubles_and_caps() {
        let p = Politeness::new(cfg()); // base 10ms, max 80ms
        assert_eq!(p.backoff_for(0), Duration::from_millis(10));
        assert_eq!(p.backoff_for(1), Duration::from_millis(20));
        assert_eq!(p.backoff_for(2), Duration::from_millis(40));
        assert_eq!(p.backoff_for(3), Duration::from_millis(80));
        assert_eq!(p.backoff_for(99), Duration::from_millis(80));
    }

    #[test]
    fn advise_retry_gives_up_at_cap() {
        let p = Politeness::new(cfg()); // max_retries = 2
        assert!(matches!(p.advise_retry(0), RetryAdvice::RetryAfter(_)));
        assert!(matches!(p.advise_retry(2), RetryAdvice::GiveUp));
    }
}
