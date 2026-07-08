# shirabe-crawler

A standard crawling orchestration layer — URL frontier, concurrent workers,
per-host politeness, schema-based extraction, and pluggable storage — that
drives a [`PageDriver`], with **shirabe's own headless browser as the canonical
backend**.

```text
  ┌──────────── Crawler (orchestration) ────────────┐
  │  frontier · workers · politeness · extract · sink │
  └───────────────────────┬───────────────────────────┘
                          │ PageDriver (the single seam)
  ┌───────────────────────▼───────────────────────────┐
  │  shirabe debug API   ←┄┄ swappable ┄┄→   mock / own │
  └─────────────────────────────────────────────────────┘
```

## Why a separate crate

shirabe is a CDP engine + debug API — the browser *backend*. Crawling is
*orchestration* on top of a backend: scheduling, throttling, extracting,
storing. Keeping the two apart means:

- **shirabe stays a pure backend.** Its public API, dependencies, and
  responsibilities don't grow.
- **The crawler is backend-agnostic.** It talks to a `PageDriver` trait. Ship a
  real crawler over shirabe today; swap in a pooled/remote driver tomorrow
  without touching orchestration logic.

This mirrors shirabe's own "backend is swappable" design (`ort` and its
providers): the crawler is to `PageDriver` what shirabe is to its browser
backend.

## Quick start

```rust
use std::sync::Arc;
use std::time::Duration;
use shirabe_crawler::{
    Crawler, MemorySink, PolitenessConfig, ShirabeDriver, ShirabeDriverConfig,
    WorkerJob, ExtractionSchema, FieldSource, FieldSpec,
};

# async fn run() -> anyhow::Result<()> {
// 1. Point at a running shirabe debug server (e.g. `shirabe debug --port 3001`).
let driver = Arc::new(ShirabeDriver::new(ShirabeDriverConfig {
    endpoint: "http://localhost:3001".into(),
    timeout: Duration::from_secs(30),
})?);

// 2. Where records go.
let sink = Arc::new(MemorySink::new());

// 3. What to extract per page.
let schema = ExtractionSchema {
    container: Some("article.post".into()),
    fields: vec![
        FieldSpec { name: "title".into(), selector: "h2".into(), source: FieldSource::Text },
        FieldSpec { name: "url".into(),   selector: "a".into(),  source: FieldSource::Attr { name: "href".into() } },
    ],
};

// 4. Build & run.
let crawler = Crawler::builder()
    .driver(driver)
    .sink(sink.clone())
    .politeness(PolitenessConfig { per_host_delay: Duration::from_secs(1), ..Default::default() })
    .concurrency(2)
    .job(WorkerJob { extract: Some(schema), follow_links: true })
    .build()?;

crawler.seed(["https://example.com/".into()]).await;
let visited = crawler.run().await?;
println!("crawled {} pages", visited);
# Ok(())
# }
```

## Pieces

| Module | Responsibility |
|---|---|
| `driver` | The `PageDriver` seam: `fetch` a URL, `evaluate` JS in the page. |
| `drivers::shirabe` | Canonical backend — drives shirabe's HTTP debug API. |
| `drivers::mock` | In-memory backend for tests / offline dev. |
| `frontier` | Priority URL queue with dedup, depth cap, scheme allowlist. |
| `politeness` | Per-host rate limit + concurrency cap, exponential backoff, retry advice. |
| `extract` | Declarative schema (`container` + `field` selectors) compiled to JS, run via the driver. |
| `link_discovery` | Absolutize `<a href>` from a page back into the frontier. |
| `sink` | `RecordSink` trait + in-memory and NDJSON file sinks. |
| `worker` | The fetch→extract→discover→sink loop, run N concurrently. |

## Configuration

`CrawlerConfig` composes the pieces:

- `frontier` — `max_depth`, `max_urls`, `allowed_schemes`
- `politeness` — `per_host_delay`, `per_host_concurrency`, `max_retries`, `backoff_*`, `respect_robots`
- `concurrency` — crawl-wide worker count (orthogonal to per-host politeness)
- `job` — what each worker does: an optional `ExtractionSchema` and whether to `follow_links`

## License

SySL-1.0 (Synthetic Source License). See [`LICENSE`](LICENSE). Portions of this
crate were generated with the assistance of AI models; the disclosure is
preserved per the license terms.
