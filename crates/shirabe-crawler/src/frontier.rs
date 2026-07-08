//! URL frontier — the work queue with dedup, priority, and seen-set.
//!
//! A standard breadth-first-with-priority frontier: URLs are pushed with a
//! numeric priority (lower = sooner), deduplicated against a seen-set, and
//! drained in priority order. The seen-set survives across the whole crawl so
//! a single URL is visited at most once per run.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use tokio::sync::Mutex;
use url::Url;

use crate::error::CrawlError;

/// A pending URL with its crawl priority.
#[derive(Debug, Clone)]
pub struct PendingUrl {
    pub url: String,
    pub priority: u32,
    /// How many link hops from a seed. Depth 0 = seed.
    pub depth: u32,
}

/// Configuration for the frontier.
#[derive(Debug, Clone)]
pub struct FrontierConfig {
    /// Hard cap on crawl depth (hops from a seed). URLs beyond this are dropped.
    pub max_depth: u32,
    /// Maximum distinct URLs the frontier will ever admit. A backstop against
    /// link-explosion crawls running away.
    pub max_urls: usize,
    /// Allowed URL schemes. Anything else is rejected at admission.
    pub allowed_schemes: Vec<String>,
}

impl Default for FrontierConfig {
    fn default() -> Self {
        Self {
            max_depth: 3,
            max_urls: 100_000,
            allowed_schemes: vec!["http".into(), "https".into()],
        }
    }
}

/// The frontier itself. Cheap to clone — the queue lives behind an `Arc<Mutex>`.
#[derive(Clone)]
pub struct Frontier {
    inner: Arc<Mutex<FrontierInner>>,
    config: FrontierConfig,
}

struct FrontierInner {
    /// priority -> bucket of pending URLs at that priority.
    queue: BTreeMap<u32, Vec<PendingUrl>>,
    /// Every URL ever admitted, normalized, to dedup across pushes.
    seen: HashSet<String>,
    /// Total URLs ever admitted (for the max_urls backstop).
    admitted: usize,
}

impl Frontier {
    pub fn new(config: FrontierConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(FrontierInner {
                queue: BTreeMap::new(),
                seen: HashSet::new(),
                admitted: 0,
            })),
            config,
        }
    }

    /// Seed the frontier with starting URLs at the given priority (default 0).
    pub async fn seed<I>(&self, urls: I)
    where
        I: IntoIterator<Item = String>,
    {
        for u in urls {
            let _ = self.push(u, 0, 0).await;
        }
    }

    /// Admit a URL. Returns the accepted [`PendingUrl`], or an error explaining
    /// why it was rejected (bad scheme, depth cap, dedup, capacity).
    pub async fn push(
        &self,
        raw: String,
        priority: u32,
        depth: u32,
    ) -> Result<PendingUrl, CrawlError> {
        let normalized = normalize_url(&raw)
            .ok_or_else(|| CrawlError::Rejected(format!("malformed url: {raw}")))?;

        let parsed = Url::parse(&normalized)
            .map_err(|e| CrawlError::Rejected(format!("bad url {raw}: {e}")))?;

        if !self
            .config
            .allowed_schemes
            .iter()
            .any(|s| s == parsed.scheme())
        {
            return Err(CrawlError::Rejected(format!(
                "scheme '{}' not allowed: {raw}",
                parsed.scheme()
            )));
        }

        if depth > self.config.max_depth {
            return Err(CrawlError::Rejected(format!(
                "depth {depth} > max_depth {}: {raw}",
                self.config.max_depth
            )));
        }

        let mut inner = self.inner.lock().await;

        if inner.seen.contains(&normalized) {
            return Err(CrawlError::Rejected(format!("already seen: {raw}")));
        }

        if inner.admitted >= self.config.max_urls {
            return Err(CrawlError::Rejected(format!(
                "max_urls ({}) reached: {raw}",
                self.config.max_urls
            )));
        }

        inner.seen.insert(normalized.clone());
        inner.admitted += 1;

        let pending = PendingUrl {
            url: normalized,
            priority,
            depth,
        };
        inner
            .queue
            .entry(priority)
            .or_default()
            .push(pending.clone());

        Ok(pending)
    }

    /// Pop the next-highest-priority URL. Returns `None` when empty.
    pub async fn pop(&self) -> Option<PendingUrl> {
        let mut inner = self.inner.lock().await;
        // BTreeMap iterates ascending by key; we want lowest priority number
        // first, which is exactly ascending order.
        let priority = *inner.queue.keys().next()?;
        let bucket = inner.queue.get_mut(&priority)?;
        let next = bucket.remove(0);
        if bucket.is_empty() {
            inner.queue.remove(&priority);
        }
        Some(next)
    }

    /// Number of URLs waiting to be crawled.
    pub async fn len(&self) -> usize {
        self.inner
            .lock()
            .await
            .queue
            .values()
            .map(|v| v.len())
            .sum()
    }

    /// Is the frontier empty?
    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }

    /// Total distinct URLs ever admitted this run.
    pub async fn admitted(&self) -> usize {
        self.inner.lock().await.admitted
    }
}

/// Normalize a URL for the seen-set: lowercase host, strip fragment, drop a
/// trailing slash on bare paths. Keeps query order as-is (callers who care
/// about query-stability should canonicalize upstream).
fn normalize_url(raw: &str) -> Option<String> {
    let mut url = Url::parse(raw.trim()).ok()?;
    url.set_fragment(None);
    if let Some(host) = url.host_str() {
        let lower = host.to_lowercase();
        url.set_host(Some(&lower)).ok()?;
    }
    // Collapse a bare "/" path to empty so "/a" and "/a/" are distinct but
    // "host/" and "host" are not.
    let mut s = url.to_string();
    if s.ends_with('/') && !s.ends_with("//") {
        s.pop();
    }
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> FrontierConfig {
        FrontierConfig {
            max_depth: 2,
            max_urls: 100,
            allowed_schemes: vec!["http".into(), "https".into()],
        }
    }

    #[tokio::test]
    async fn dedups_seen_urls() {
        let f = Frontier::new(cfg());
        f.seed(["https://example.com/".into(), "https://EXAMPLE.com".into()])
            .await;
        // Both normalize to the same string → only one admitted.
        assert_eq!(f.admitted().await, 1);
    }

    #[tokio::test]
    async fn rejects_bad_scheme() {
        let f = Frontier::new(cfg());
        let err = f.push("file:///etc/passwd".into(), 0, 0).await.unwrap_err();
        assert!(matches!(err, CrawlError::Rejected(_)));
    }

    #[tokio::test]
    async fn pops_in_priority_order() {
        let f = Frontier::new(cfg());
        f.push("https://a.example/".into(), 5, 0).await.unwrap();
        f.push("https://b.example/".into(), 1, 0).await.unwrap();
        f.push("https://c.example/".into(), 3, 0).await.unwrap();

        assert_eq!(f.pop().await.unwrap().url, "https://b.example");
        assert_eq!(f.pop().await.unwrap().url, "https://c.example");
        assert_eq!(f.pop().await.unwrap().url, "https://a.example");
        assert!(f.pop().await.is_none());
    }

    #[tokio::test]
    async fn enforces_depth_cap() {
        let f = Frontier::new(cfg()); // max_depth = 2
        let err = f.push("https://x.example/".into(), 0, 3).await.unwrap_err();
        assert!(matches!(err, CrawlError::Rejected(_)));
    }
}
