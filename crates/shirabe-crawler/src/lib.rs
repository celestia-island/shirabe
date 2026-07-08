//! `shirabe-crawler` — a standard crawling orchestration layer over a
//! [`PageDriver`], with shirabe's headless browser as the canonical backend.
//!
//! ```text
//!   ┌──────────── Crawler (orchestration) ────────────┐
//!   │  frontier · workers · politeness · extract · sink │
//!   └───────────────────────┬───────────────────────────┘
//!                           │ PageDriver (the single seam)
//!   ┌───────────────────────▼───────────────────────────┐
//!   │  shirabe debug API   ←┄┄ swappable ┄┄→   mock / own │
//!   └─────────────────────────────────────────────────────┘
//! ```
//!
//! The crawler never touches a browser directly: it schedules URLs, throttles,
//! extracts via a JS schema run through the driver, and sinks records. Swap the
//! `PageDriver` impl and the rest is unchanged.

mod driver;
mod drivers;
mod error;
mod extract;
mod frontier;
pub mod link_discovery;
mod politeness;
mod sink;
mod worker;

use std::sync::Arc;

use tokio::sync::Semaphore;
use tracing::info;

pub use driver::{FetchedPage, PageDriver};
pub use drivers::{MockDriver, ShirabeDriver, ShirabeDriverConfig};
pub use error::CrawlError;
pub use extract::{ExtractionSchema, FieldSource, FieldSpec, Record};
pub use frontier::{Frontier, FrontierConfig, PendingUrl};
pub use politeness::{GateVerdict, Politeness, PolitenessConfig, RetryAdvice};
pub use sink::{MemorySink, NdjsonSink, RecordSink};
pub use worker::{WorkerJob, WorkerOutcome};

/// Top-level crawl configuration.
#[derive(Clone)]
pub struct CrawlerConfig {
    pub frontier: FrontierConfig,
    pub politeness: PolitenessConfig,
    /// How many URLs are fetched concurrently across the whole crawl.
    pub concurrency: usize,
    /// What each worker does per page.
    pub job: WorkerJob,
}

impl Default for CrawlerConfig {
    fn default() -> Self {
        Self {
            frontier: FrontierConfig::default(),
            politeness: PolitenessConfig::default(),
            concurrency: 2,
            job: WorkerJob::default(),
        }
    }
}

/// A running crawl handle. Build with [`Crawler::builder`].
pub struct Crawler {
    config: CrawlerConfig,
    frontier: Frontier,
    driver: Arc<dyn PageDriver>,
    sink: Arc<dyn RecordSink>,
}

impl Crawler {
    /// Fluent builder entry point.
    pub fn builder() -> CrawlerBuilder {
        CrawlerBuilder::default()
    }

    /// Seed starting URLs.
    pub async fn seed<I>(&self, urls: I)
    where
        I: IntoIterator<Item = String>,
    {
        self.frontier.seed(urls).await;
    }

    /// Run the crawl to completion (frontier drains or limit hit). Returns how
    /// many pages were attempted. Workers run concurrently up to `concurrency`.
    pub async fn run(&self) -> Result<usize, CrawlError> {
        let (_stop_tx, stop_rx) = tokio::sync::watch::channel(false);
        let semaphore = Arc::new(Semaphore::new(self.config.concurrency.max(1)));

        let ctx = Arc::new(worker::CrawlContext {
            frontier: self.frontier.clone(),
            driver: self.driver.clone(),
            politeness: Politeness::new(self.config.politeness.clone()),
            sink: self.sink.clone(),
            job: self.job(),
            concurrency: semaphore,
            stop: stop_rx,
        });

        let worker_count = self.config.concurrency.max(1);
        let mut handles = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let ctx = ctx.clone();
            handles.push(tokio::spawn(async move {
                worker::run_worker(ctx).await;
            }));
        }
        for h in handles {
            let _ = h.await;
        }
        let admitted = self.frontier.admitted().await;
        info!(admitted, "crawl finished");
        Ok(admitted)
    }

    fn job(&self) -> WorkerJob {
        self.config.job.clone()
    }

    /// Stop signal (currently fires on `run` exit; reserved for future
    /// time-limit / graceful-stop support).
    #[allow(dead_code)]
    fn _stop_handle(&self) {}
}

/// Builder for [`Crawler`].
#[derive(Default)]
pub struct CrawlerBuilder {
    config: CrawlerConfig,
    driver: Option<Arc<dyn PageDriver>>,
    sink: Option<Arc<dyn RecordSink>>,
}

impl CrawlerBuilder {
    pub fn frontier(mut self, c: FrontierConfig) -> Self {
        self.config.frontier = c;
        self
    }
    pub fn politeness(mut self, c: PolitenessConfig) -> Self {
        self.config.politeness = c;
        self
    }
    pub fn concurrency(mut self, n: usize) -> Self {
        self.config.concurrency = n;
        self
    }
    pub fn job(mut self, j: WorkerJob) -> Self {
        self.config.job = j;
        self
    }
    pub fn driver(mut self, d: Arc<dyn PageDriver>) -> Self {
        self.driver = Some(d);
        self
    }
    pub fn sink(mut self, s: Arc<dyn RecordSink>) -> Self {
        self.sink = Some(s);
        self
    }
    pub fn build(self) -> Result<Crawler, CrawlError> {
        let driver = self
            .driver
            .ok_or_else(|| CrawlError::Rejected("no PageDriver configured".into()))?;
        let sink = self
            .sink
            .ok_or_else(|| CrawlError::Rejected("no RecordSink configured".into()))?;
        Ok(Crawler {
            frontier: Frontier::new(self.config.frontier.clone()),
            config: self.config,
            driver,
            sink,
        })
    }
}
