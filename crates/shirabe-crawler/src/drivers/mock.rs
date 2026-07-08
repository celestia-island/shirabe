//! In-memory mock driver — for unit tests and offline development.
//!
//! Serves canned HTML from a map of URL → HTML string, and evaluates JS
//! against a tiny fake DOM that knows only `document.querySelectorAll` for the
//! shapes the crawler's JS snippets use. Good enough to exercise the
//! orchestration layer end-to-end without a browser.

use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::driver::{FetchedPage, PageDriver};
use crate::error::CrawlError;

/// A mock driver preloaded with URL → HTML pairs.
pub struct MockDriver {
    pages: Mutex<HashMap<String, String>>,
    /// Captures every fetch URL, in order, for assertions.
    pub fetched: Mutex<Vec<String>>,
}

impl MockDriver {
    pub fn new() -> Self {
        Self {
            pages: Mutex::new(HashMap::new()),
            fetched: Mutex::new(Vec::new()),
        }
    }

    pub async fn with(self, url: impl Into<String>, html: impl Into<String>) -> Self {
        self.pages.lock().await.insert(url.into(), html.into());
        self
    }
}

impl Default for MockDriver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PageDriver for MockDriver {
    async fn fetch(&self, url: &str) -> Result<FetchedPage, CrawlError> {
        self.fetched.lock().await.push(url.to_string());
        let pages = self.pages.lock().await;
        let html = pages
            .get(url)
            .cloned()
            .unwrap_or_else(|| format!("<html><body><h1>{url}</h1></body></html>"));
        Ok(FetchedPage {
            final_url: url.to_string(),
            status: 200,
            html,
            title: Some(url.to_string()),
        })
    }

    async fn evaluate(&self, expression: &str, page: &FetchedPage) -> Result<Value, CrawlError> {
        // Very small fake evaluator: support the two shapes the crawler emits.
        // 1. Anchor discovery: returns a[href] hrefs.
        // 2. Extraction schema: container.querySelectorAll → fields. We support
        //    a trivial single-selector, text-content case.
        if expression.contains("querySelectorAll('a[href]')") {
            let hrefs = extract_anchors(&page.html, &page.final_url);
            return Ok(json!(hrefs));
        }
        // For extraction, return whatever container count we can find, with
        // placeholder field values. This is enough to test the plumbing.
        Ok(json!([{}]))
    }
}

/// Crude `href` extraction from static HTML, resolving against the base URL.
/// Good enough for the mock — real crawling runs this JS in a real browser.
fn extract_anchors(html: &str, base: &str) -> Vec<String> {
    let base_url = url::Url::parse(base).ok();
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(idx) = rest.find("href") {
        rest = &rest[idx + 4..];
        // skip '=' and whitespace/quote
        let after_eq = rest.trim_start();
        let after_eq = after_eq.strip_prefix('=').unwrap_or(after_eq);
        let after_eq = after_eq.trim_start();
        let (href, _remaining) = match after_eq.chars().next() {
            Some('"') => {
                let s = &after_eq[1..];
                match s.find('"') {
                    Some(end) => (&s[..end], &s[end + 1..]),
                    None => break,
                }
            }
            Some('\'') => {
                let s = &after_eq[1..];
                match s.find('\'') {
                    Some(end) => (&s[..end], &s[end + 1..]),
                    None => break,
                }
            }
            _ => {
                let end = after_eq
                    .find(|c: char| c.is_whitespace() || c == '>')
                    .unwrap_or(after_eq.len());
                (&after_eq[..end], &after_eq[end..])
            }
        };
        if let Some(b) = base_url.as_ref() {
            if let Ok(abs) = b.join(href) {
                if matches!(abs.scheme(), "http" | "https") {
                    out.push(abs.to_string());
                }
            }
        }
        rest = _remaining;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn serves_canned_html() {
        let d = MockDriver::new().with("https://x/", "<p>hi</p>").await;
        let p = d.fetch("https://x/").await.unwrap();
        assert_eq!(p.html, "<p>hi</p>");
    }

    #[test]
    fn extracts_absolute_anchors() {
        let hrefs = extract_anchors(
            r#"<a href="/a">1</a><a href="https://y/z">2</a>"#,
            "https://x/",
        );
        assert_eq!(hrefs, vec!["https://x/a", "https://y/z"]);
    }
}
