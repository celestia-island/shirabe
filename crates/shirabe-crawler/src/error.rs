//! Error types for the crawler.

use thiserror::Error;

/// All crawler failures funnel through here.
#[derive(Debug, Error)]
pub enum CrawlError {
    /// A page failed to load (network, backend, timeout).
    #[error("fetch failed for {url}: {source}")]
    Fetch {
        url: String,
        #[source]
        source: anyhow::Error,
    },

    /// The frontier rejected a URL (bad scheme, disallowed by robots, etc.).
    #[error("url rejected: {0}")]
    Rejected(String),

    /// Extraction produced no usable records where at least one was expected.
    #[error("extraction yielded no records for {0}")]
    EmptyExtraction(String),

    /// A storage sink rejected a record.
    #[error("sink error: {0}")]
    Sink(String),

    /// The crawl was stopped (limit hit, shutdown, or cancelled).
    #[error("crawl stopped: {0}")]
    Stopped(String),
}

impl CrawlError {
    /// Whether this error is transient and the URL could be retried.
    pub fn is_retryable(&self) -> bool {
        matches!(self, CrawlError::Fetch { .. })
    }
}
