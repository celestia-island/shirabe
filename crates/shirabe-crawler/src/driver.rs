//! Page driver abstraction — how the crawler obtains a navigable page.
//!
//! This is the **single seam** between the crawling orchestration layer and
//! whatever browser backend actually renders pages. The core crawler talks
//! only to [`PageDriver`]; it never knows whether a page is a real headless
//! Chromium, a pool of them, or an in-memory mock. Mirrors shirabe's own
//! "backend is swappable" philosophy (cf. `ort` and its providers): the crawler
//! is to `PageDriver` what shirabe is to its browser backend.

use async_trait::async_trait;

use crate::error::CrawlError;

/// A fetched page ready for extraction.
#[derive(Debug, Clone)]
pub struct FetchedPage {
    /// Final URL after any redirects.
    pub final_url: String,
    /// HTTP-ish status code when available (0 when the backend doesn't surface one).
    pub status: u16,
    /// The page's rendered HTML.
    pub html: String,
    /// Optional document title.
    pub title: Option<String>,
}

/// How a crawler obtains and renders pages.
///
/// Implementations own the browser backend and decide its concurrency model —
/// one shared session, a pool, remote, local. The orchestration layer is
/// intentionally unaware of this so that "how many browsers" stays a backend
/// concern, not a crawler concern.
#[async_trait]
pub trait PageDriver: Send + Sync {
    /// Fetch `url`, waiting for it to settle, and return the rendered page.
    ///
    /// Implementations should apply per-driver politeness (waiting for network
    /// idle, timeouts) here. The orchestration layer applies crawl-wide
    /// politeness (rate limiting, backoff) on top.
    async fn fetch(&self, url: &str) -> Result<FetchedPage, CrawlError>;

    /// Evaluate a JavaScript expression in the context of a page already
    /// fetched via [`fetch`](Self::fetch), returning its JSON-serializable
    /// result. Drivers that cannot execute JS should return `Err` so callers
    /// degrade gracefully (skip extraction / link discovery).
    async fn evaluate(
        &self,
        expression: &str,
        page: &FetchedPage,
    ) -> Result<serde_json::Value, CrawlError>;
}
