# Crawling orchestration (shirabe-crawler)

`shirabe-crawler` is a standard crawling orchestration layer — URL frontier,
concurrent workers, per-host politeness, schema-based extraction, and pluggable
storage — that drives a `PageDriver`, with **shirabe's own headless browser as
the canonical backend**.

## Why a separate crate

shirabe is a CDP engine + debug API — the browser *backend*. Crawling is
*orchestration* on top of a backend. Keeping them apart means shirabe stays a
pure backend, and the crawler is backend-agnostic — swap the `PageDriver` impl
without touching orchestration. This mirrors shirabe's own swappable-backend
design (`ort` and its providers).

## Quick start

```rust
use std::sync::Arc;
use std::time::Duration;
use shirabe_crawler::{
    Crawler, MemorySink, PolitenessConfig, ShirabeDriver, ShirabeDriverConfig,
    WorkerJob, ExtractionSchema, FieldSource, FieldSpec,
};

# async fn run() -> anyhow::Result<()> {
let driver = Arc::new(ShirabeDriver::new(ShirabeDriverConfig {
    endpoint: "http://localhost:3001".into(),
    timeout: Duration::from_secs(30),
})?);

let sink = Arc::new(MemorySink::new());

let schema = ExtractionSchema {
    container: Some("article.post".into()),
    fields: vec![
        FieldSpec { name: "title".into(), selector: "h2".into(), source: FieldSource::Text },
        FieldSpec { name: "url".into(),   selector: "a".into(),  source: FieldSource::Attr { name: "href".into() } },
    ],
};

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

## Modules

| Module | Responsibility |
|---|---|
| `driver` | The `PageDriver` seam: `fetch` a URL, `evaluate` JS. |
| `drivers::shirabe` | Canonical backend — shirabe's HTTP debug API. |
| `drivers::mock` | In-memory backend for tests. |
| `frontier` | Priority URL queue, dedup, depth cap, scheme allowlist. |
| `politeness` | Per-host rate limit + concurrency, exponential backoff, retry advice. |
| `extract` | Declarative schema compiled to JS, run via the driver. |
| `link_discovery` | Absolutize `<a href>` back into the frontier. |
| `sink` | `RecordSink` trait + in-memory and NDJSON sinks. |
| `worker` | The fetch→extract→discover→sink loop, N concurrent. |

## Decoupling boundary

- shirabe's core (`src/`) is unchanged; only the root `Cargo.toml` gains a
  workspace stanza.
- This crate depends only on shirabe's **public** API + HTTP debug API — no
  `pub(crate)` symbols.
- Remove `crates/shirabe-crawler/` to fully revert; shirabe has no residue.

## License

SySL-1.0 (Synthetic Source License). Portions were generated with AI assistance;
disclosure is preserved per the license terms.
