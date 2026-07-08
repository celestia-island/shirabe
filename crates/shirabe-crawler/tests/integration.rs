//! End-to-end orchestration test against the in-memory [`MockDriver`].
//!
//! Exercises the full path a real crawl takes — seed → fetch → link discovery
//! → frontier growth → extraction → sink — without touching a browser. This is
//! the integration seam: if any module's contract drifts, this test breaks.

use std::sync::Arc;

use shirabe_crawler::{
    Crawler, ExtractionSchema, FieldSource, FieldSpec, MemorySink, MockDriver, PolitenessConfig,
    WorkerJob,
};
use std::time::Duration;

fn fast_politeness() -> PolitenessConfig {
    PolitenessConfig {
        per_host_delay: Duration::from_millis(1),
        per_host_concurrency: 4,
        max_retries: 1,
        backoff_base: Duration::from_millis(1),
        backoff_max: Duration::from_millis(4),
        respect_robots: false,
    }
}

#[tokio::test]
async fn crawl_discovers_links_and_drains() {
    // A tiny 3-page site: index → about → contact. Mock keys use the
    // *normalized* form the frontier stores (no trailing slash on bare hosts),
    // matching how a real server treats "/" and "" equivalently here.
    let driver = MockDriver::new()
        .with(
            "https://site.example",
            r#"<html><body>
                 <a href="/about">About</a>
                 <a href="https://site.example/contact">Contact</a>
                 <a href="mailto:x@y.com">Mail</a>
               </body></html>"#,
        )
        .await
        .with(
            "https://site.example/about",
            r#"<html><body><a href="/contact">Contact</a></body></html>"#,
        )
        .await
        .with(
            "https://site.example/contact",
            r#"<html><body><p>end of the line</p></body></html>"#,
        )
        .await;

    let sink = Arc::new(MemorySink::new());

    let crawler = Crawler::builder()
        .driver(Arc::new(driver))
        .sink(sink.clone())
        .politeness(fast_politeness())
        .concurrency(2)
        // Follow links but do not extract — pure crawl.
        .job(WorkerJob {
            extract: None,
            follow_links: true,
        })
        .build()
        .unwrap();

    crawler.seed(["https://site.example".into()]).await;
    let admitted = crawler.run().await.unwrap();

    // All three pages admitted; mailto dropped; nothing revisited.
    assert_eq!(admitted, 3);
}

#[tokio::test]
async fn crawl_extracts_records_via_schema() {
    let driver = MockDriver::new()
        .with(
            "https://shop.example",
            r#"<html><body>
                 <div class="item"><h2>Widget</h2><a href="/w">buy</a></div>
               </body></html>"#,
        )
        .await;

    let schema = ExtractionSchema {
        container: Some("div.item".into()),
        fields: vec![
            FieldSpec {
                name: "title".into(),
                selector: "h2".into(),
                source: FieldSource::Text,
            },
            FieldSpec {
                name: "link".into(),
                selector: "a".into(),
                source: FieldSource::Attr {
                    name: "href".into(),
                },
            },
        ],
    };

    let sink = Arc::new(MemorySink::new());
    let crawler = Crawler::builder()
        .driver(Arc::new(driver))
        .sink(sink.clone())
        .politeness(fast_politeness())
        .concurrency(1)
        .job(WorkerJob {
            extract: Some(schema),
            follow_links: false,
        })
        .build()
        .unwrap();

    crawler.seed(["https://shop.example".into()]).await;
    crawler.run().await.unwrap();

    // The mock evaluator returns placeholder records; the point of this test is
    // that extraction ran and the sink received at least one record, proving
    // the schema→JS→evaluate→sink plumbing is wired end to end.
    assert!(
        sink.len().await >= 1,
        "expected at least one extracted record"
    );
}

#[tokio::test]
async fn crawl_retries_then_succeeds_when_page_appears() {
    // A driver that fails the first fetch for a URL then serves it. Simulates a
    // transient network blip recovering.
    use async_trait::async_trait;
    use shirabe_crawler::{CrawlError, FetchedPage, PageDriver};
    use std::sync::atomic::{AtomicU32, Ordering};

    struct FlakyDriver {
        attempts: AtomicU32,
        html: String,
    }

    #[async_trait]
    impl PageDriver for FlakyDriver {
        async fn fetch(&self, _url: &str) -> Result<FetchedPage, CrawlError> {
            let n = self.attempts.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                return Err(CrawlError::Fetch {
                    url: _url.into(),
                    source: anyhow::anyhow!("simulated transient failure"),
                });
            }
            Ok(FetchedPage {
                final_url: _url.into(),
                status: 200,
                html: self.html.clone(),
                title: None,
            })
        }
        async fn evaluate(
            &self,
            _expression: &str,
            _page: &FetchedPage,
        ) -> Result<serde_json::Value, CrawlError> {
            Ok(serde_json::json!([]))
        }
    }

    let driver = Arc::new(FlakyDriver {
        attempts: AtomicU32::new(0),
        html: "<html></html>".into(),
    });
    let sink = Arc::new(MemorySink::new());

    let crawler = Crawler::builder()
        .driver(driver)
        .sink(sink.clone())
        .politeness(fast_politeness())
        .concurrency(1)
        .build()
        .unwrap();

    crawler.seed(["https://flaky.example/".into()]).await;
    let admitted = crawler.run().await.unwrap();
    // Even though the first attempt failed, the URL was admitted and retried.
    assert_eq!(admitted, 1);
}
