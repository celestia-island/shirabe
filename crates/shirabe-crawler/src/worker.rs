//! Worker loop — drains the frontier, fetches, extracts, discovers links, sinks.
//!
//! Each worker is a task that pops a URL, asks the politeness gate, drives the
//! [`PageDriver`] to render it, runs any extraction schema, optionally follows
//! discovered links, and writes records to the sink. The orchestrator spins up
//! `N` of these; they share one frontier, one politeness gate, one driver, one
//! sink. `N` is the crawl-wide concurrency cap — orthogonal to how many
//! browsers the driver actually owns (that's the driver's secret).

use std::sync::Arc;

use tokio::sync::Semaphore;
use tokio::time::sleep;
use tracing::{debug, warn};

use crate::driver::{FetchedPage, PageDriver};
use crate::extract::{ExtractionSchema, parse_eval_result};
use crate::frontier::Frontier;
use crate::link_discovery;
use crate::politeness::{GateVerdict, Politeness, RetryAdvice};
use crate::sink::RecordSink;

/// What a worker should do with each page it fetches.
#[derive(Clone, Default)]
pub struct WorkerJob {
    /// Optional extraction schema. If `None`, the page is fetched for link
    /// discovery only (a pure crawl, no data extraction).
    pub extract: Option<ExtractionSchema>,
    /// When true, discovered `<a href>` links (same-scheme, same-or-sub host)
    /// are pushed back into the frontier for further crawling.
    pub follow_links: bool,
}

/// One unit of work produced by a worker for a single URL: stats + the page,
/// so the orchestrator (or a caller) can react.
pub struct WorkerOutcome {
    pub url: String,
    pub records: usize,
    pub links_discovered: usize,
    pub retries: u32,
    pub ok: bool,
}

/// Shared crawl context handed to every worker.
pub(crate) struct CrawlContext {
    pub frontier: Frontier,
    pub driver: Arc<dyn PageDriver>,
    pub politeness: Politeness,
    pub sink: Arc<dyn RecordSink>,
    pub job: WorkerJob,
    /// A crawl-wide concurrency cap, separate from per-host politeness.
    pub concurrency: Arc<Semaphore>,
    /// Cooperative shutdown signal.
    pub stop: tokio::sync::watch::Receiver<bool>,
}

/// Run one worker until the frontier drains or `stop` fires.
pub(crate) async fn run_worker(ctx: Arc<CrawlContext>) {
    loop {
        if *ctx.stop.borrow() {
            break;
        }

        let Some(pending) = ctx.frontier.pop().await else {
            // Frontier empty. Bail out (orchestrator may re-seed).
            break;
        };

        // Crawl-wide concurrency permit (released on scope exit via _permit).
        let _permit = match ctx.concurrency.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => break, // semaphore closed = shutdown
        };

        let outcome = crawl_one(&ctx, pending.url.clone(), pending.depth).await;
        if !outcome.ok {
            debug!(url = %outcome.url, retries = outcome.retries, "crawl failed");
        }
        // Let the orchestrator observe; here we just continue.
        let _ = outcome;
    }
}

/// Crawl a single URL with retries, politeness, extraction, and link discovery.
async fn crawl_one(ctx: &CrawlContext, url: String, depth: u32) -> WorkerOutcome {
    let mut retries = 0u32;
    loop {
        // Politeness gate.
        match ctx.politeness.check(&url).await {
            GateVerdict::Allow => {}
            GateVerdict::Wait(d) => {
                sleep(d).await;
                continue;
            }
            GateVerdict::Deny(reason) => {
                warn!(url = %url, %reason, "politely denied");
                ctx.politeness.release(&url, false).await;
                return failed(url, retries, &reason);
            }
        }

        // Fetch.
        match ctx.driver.fetch(&url).await {
            Ok(page) => {
                ctx.politeness.release(&url, true).await;
                return process_page(ctx, url, depth, page, retries).await;
            }
            Err(e) => {
                ctx.politeness.release(&url, false).await;
                match ctx.politeness.advise_retry(retries) {
                    RetryAdvice::RetryAfter(d) => {
                        warn!(url = %url, error = %e, retry = retries + 1, "fetch failed, backing off");
                        retries += 1;
                        sleep(d).await;
                        continue;
                    }
                    RetryAdvice::GiveUp => {
                        warn!(url = %url, error = %e, "fetch failed, giving up");
                        return failed(url, retries, &e.to_string());
                    }
                }
            }
        }
    }
}

async fn process_page(
    ctx: &CrawlContext,
    url: String,
    depth: u32,
    page: FetchedPage,
    retries: u32,
) -> WorkerOutcome {
    let mut records_written = 0usize;
    let mut links_discovered = 0usize;

    // Extraction (schema → JS → driver.evaluate). When the driver can't
    // evaluate JS (e.g. a static HTTP mock), we degrade gracefully: no records.
    if let Some(schema) = ctx.job.extract.as_ref() {
        if let Ok(raw_records) = ctx.driver.evaluate(schema.to_js().as_str(), &page).await {
            let records = parse_eval_result(raw_records);
            for rec in records {
                if ctx.sink.write(rec).await.is_ok() {
                    records_written += 1;
                }
            }
        }
    }

    // Link discovery.
    if ctx.job.follow_links {
        if let Ok(links) = ctx.driver.evaluate(link_discovery::ANCHOR_JS, &page).await {
            for href in link_discovery::absolutize(&url, links) {
                // Push at one greater depth. Errors (dedup, depth cap) are
                // expected and just mean "we won't crawl it" — not failures.
                if ctx
                    .frontier
                    .push(href, 0, depth.saturating_add(1))
                    .await
                    .is_ok()
                {
                    links_discovered += 1;
                }
            }
        }
    }

    WorkerOutcome {
        url,
        records: records_written,
        links_discovered,
        retries,
        ok: true,
    }
}

fn failed(url: String, retries: u32, reason: &str) -> WorkerOutcome {
    let _ = reason; // surfaced via tracing in caller; kept for clarity.
    WorkerOutcome {
        url,
        records: 0,
        links_discovered: 0,
        retries,
        ok: false,
    }
}
