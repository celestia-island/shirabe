//! `shirabe` HTTP driver — drives shirabe's own headless browser over its
//! HTTP debug API (`/navigate`, `/evaluate`, `/dom`).
//!
//! This is the canonical backend: a single shared shirabe debug server renders
//! every page. Concurrency here is **serialized** (one browser session), which
//! is why crawl-wide concurrency is throttled by the orchestrator's semaphore
//! and per-host politeness — not by spinning up browsers here. If a future
//! driver wants true browser-level parallelism, it can pool multiple servers
//! behind the same trait; the crawler won't notice.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::driver::{FetchedPage, PageDriver};
use crate::error::CrawlError;

/// Where the shirabe debug API lives and how patient we are.
#[derive(Debug, Clone)]
pub struct ShirabeDriverConfig {
    /// Base URL of the running shirabe debug server, e.g. `http://localhost:3001`.
    pub endpoint: String,
    /// Per-request timeout for navigate + evaluate calls.
    pub timeout: Duration,
}

impl Default for ShirabeDriverConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:3001".into(),
            timeout: Duration::from_secs(30),
        }
    }
}

/// A [`PageDriver`] backed by a shirabe debug server.
pub struct ShirabeDriver {
    cfg: ShirabeDriverConfig,
    client: reqwest::Client,
    /// The last navigated URL, remembered so `evaluate` can complain if it's
    /// called before any `fetch`. Cheap to clone into the Arc below.
    state: Arc<tokio::sync::Mutex<Option<String>>>,
}

impl ShirabeDriver {
    pub fn new(cfg: ShirabeDriverConfig) -> Result<Self, CrawlError> {
        let client = reqwest::Client::builder()
            .timeout(cfg.timeout)
            .build()
            .map_err(|e| CrawlError::Fetch {
                url: cfg.endpoint.clone(),
                source: anyhow::anyhow!("build http client: {e}"),
            })?;
        Ok(Self {
            cfg,
            client,
            state: Arc::new(tokio::sync::Mutex::new(None)),
        })
    }

    fn url(&self, path: &str) -> String {
        let base = self.cfg.endpoint.trim_end_matches('/');
        format!("{base}{path}")
    }
}

#[async_trait]
impl PageDriver for ShirabeDriver {
    async fn fetch(&self, url: &str) -> Result<FetchedPage, CrawlError> {
        // 1. Navigate.
        let nav_body = serde_json::json!({ "url": url });
        let nav: ApiResponse<NavigateData> = post(&self.client, self.url("/navigate"), nav_body)
            .await
            .map_err(|e| CrawlError::Fetch {
                url: url.into(),
                source: anyhow::anyhow!("navigate: {e}"),
            })?;

        let title = nav.into_data().ok().map(|d| d.title);

        // 2. Grab the rendered HTML via DOM query on `html` (full document).
        let dom: ApiResponse<Value> = self
            .client
            .get(self.url("/dom"))
            .json(&DomQuery {
                selector: "html".into(),
                all: Some(false),
            })
            .send()
            .await
            .map_err(|e| CrawlError::Fetch {
                url: url.into(),
                source: anyhow::anyhow!("dom query: {e}"),
            })?
            .json()
            .await
            .map_err(|e| CrawlError::Fetch {
                url: url.into(),
                source: anyhow::anyhow!("dom decode: {e}"),
            })?;

        let html = dom
            .into_data()
            .ok()
            .and_then(|v| v.get("html").and_then(|h| h.as_str()).map(str::to_owned))
            .unwrap_or_default();

        *self.state.lock().await = Some(url.to_string());

        Ok(FetchedPage {
            final_url: url.to_string(),
            status: 200,
            html,
            title,
        })
    }

    async fn evaluate(&self, expression: &str, _page: &FetchedPage) -> Result<Value, CrawlError> {
        let has_page = self.state.lock().await.is_some();
        if !has_page {
            return Err(CrawlError::Fetch {
                url: "(no page)".into(),
                source: anyhow::anyhow!("evaluate called before any fetch"),
            });
        }
        let body = serde_json::json!({ "expression": expression });
        let resp: ApiResponse<EvaluateData> = post(&self.client, self.url("/evaluate"), body)
            .await
            .map_err(|e| CrawlError::Fetch {
                url: "(evaluate)".into(),
                source: anyhow::anyhow!("evaluate: {e}"),
            })?;
        let data = resp.into_data().map_err(|e| CrawlError::Fetch {
            url: "(evaluate)".into(),
            source: anyhow::anyhow!("evaluate: {e}"),
        })?;
        Ok(data.result)
    }
}

// ── shirabe API wire shapes ────────────────────────────────────────────────
//
// These mirror the request/response structs in shirabe's engine.rs. Kept as
// minimal local copies so this crate compiles without depending on shirabe's
// internal types — it only needs the wire format.

#[derive(Debug, Serialize)]
struct DomQuery {
    selector: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    all: Option<bool>,
}

/// shirabe's standard `{ ok, data, error }` envelope.
#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    ok: bool,
    /// Absent when the API returns `ok: false`; serde treats a missing field
    /// as `None` for `Option` without requiring `T: Default`.
    data: Option<T>,
    #[serde(default)]
    error: Option<String>,
}

impl<T> ApiResponse<T> {
    /// Unwrap into the payload, or surface the API error.
    fn into_data(self) -> Result<T, String> {
        if self.ok {
            self.data.ok_or_else(|| "ok=true but no data".to_string())
        } else {
            Err(self.error.unwrap_or_else(|| "unknown error".into()))
        }
    }
}

#[derive(Debug, Deserialize)]
struct NavigateData {
    title: String,
}

#[derive(Debug, Deserialize)]
struct EvaluateData {
    result: Value,
}

async fn post<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: String,
    body: Value,
) -> Result<T, String> {
    client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<T>()
        .await
        .map_err(|e| e.to_string())
}
