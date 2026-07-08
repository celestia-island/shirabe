# feat: add `shirabe-crawler` — standard crawling orchestration over a `PageDriver`

## What

A standard crawling layer — URL frontier, concurrent workers, per-host
politeness, schema-based extraction, pluggable storage — that drives a
`PageDriver`, with shirabe's own headless browser as the canonical backend.

Lives in a **new workspace member** `crates/shirabe-crawler/`.

## Why a separate crate (the decoupling argument)

shirabe is a CDP engine + debug API — the browser *backend*. Crawling is
*orchestration* on top of a backend. Keeping them apart:

- **shirabe stays a pure backend.** Its public API, dependencies, and
  responsibilities don't grow.
- **The crawler is backend-agnostic.** It talks to a `PageDriver` trait. Today
  it drives shirabe over HTTP; tomorrow a pooled/remote driver can slot in
  without touching orchestration logic.

This mirrors shirabe's own swappable-backend design (`ort` and its providers):
**the crawler is to `PageDriver` what shirabe is to its browser backend.**

## Decoupling boundary (for review)

| Constraint | Status |
|---|---|
| shirabe core `src/*.rs` unchanged | ✅ `git diff --name-only -- src/` is empty |
| Only root `Cargo.toml` touched | ✅ +7 lines (workspace stanza only; `[package]`/`[dependencies]` untouched) |
| Depends only on shirabe's **public** API + HTTP debug API | ✅ no `pub(crate)` symbols referenced |
| Fully revertible | ✅ `rm -r crates/shirabe-crawler/` + drop the workspace stanza |
| Same Rust edition / rust-version / license as shirabe | ✅ edition 2024, rust 1.85, SySL-1.0 |

## Architecture

```text
  ┌──────────── Crawler (orchestration) ────────────┐
  │  frontier · workers · politeness · extract · sink │
  └───────────────────────┬───────────────────────────┘
                          │ PageDriver (the single seam)
  ┌───────────────────────▼───────────────────────────┐
  │  shirabe debug API   ←┄┄ swappable ┄┄→   mock / own │
  └─────────────────────────────────────────────────────┘
```

| Module | Responsibility |
|---|---|
| `driver` | `PageDriver` trait: `fetch` a URL, `evaluate` JS in the page. |
| `drivers::shirabe` | Canonical backend — shirabe's HTTP debug API (`/navigate`, `/dom`, `/evaluate`). |
| `drivers::mock` | In-memory backend for tests / offline dev. |
| `frontier` | Priority URL queue with dedup, depth cap, scheme allowlist. |
| `politeness` | Per-host rate limit + concurrency cap, exponential backoff, retry advice. |
| `extract` | Declarative schema (`container` + field selectors) compiled to JS, run via the driver. |
| `link_discovery` | Absolutize `<a href>` from a page back into the frontier. |
| `sink` | `RecordSink` trait + in-memory and NDJSON file sinks. |
| `worker` | The fetch→extract→discover→sink loop, run N concurrently. |

## Design notes

- **Extraction runs in the page, not in Rust.** A schema compiles to a standard
  DOM-API JS snippet executed via the driver's `evaluate`. This leans on
  shirabe's core capability (it renders the DOM anyway) and keeps the crate free
  of any HTML-parsing dependency.
- **Concurrency is crawl-wide, orthogonal to the driver's browser model.**
  shirabe's debug server is a single browser session today; the crawler's
  worker count is throttled by a semaphore + per-host politeness, so it never
  assumes a browser-per-worker. A future pooled driver raises real parallelism
  without changing the crawler.
- **No heavy dependencies added.** `tokio`, `serde`, `reqwest`, `url`, `anyhow`,
  `thiserror`, `async-trait`, `tracing` — all already in shirabe's ecosystem.

## Verification

Built and verified locally with Rust 1.96.1 (MSVC), matching shirabe's CI gate:

```
cargo fmt -p shirabe-crawler -- --check              # PASS
cargo clippy -p shirabe-crawler --all-features --all-targets -- -D warnings   # PASS
cargo test  -p shirabe-crawler --all-features        # 20 unit + 3 integration = 23 PASS
```

The unit + integration suite exercises the full path (seed → fetch → link
discovery → frontier growth → extraction → sink → retry) against the in-memory
`MockDriver`, **no browser required**.

### Honest scope of verification

- ✅ Core orchestration, frontier, politeness, extraction, sinks — fully tested
  via `MockDriver`.
- ✅ `ShirabeDriver` compiles clean and its wire shapes mirror
  `engine.rs`'s `ApiResponse`/`NavigateRequest`/`EvaluateRequest`.
- ⚠️ `ShirabeDriver` against a **live** shirabe debug server was **not** run
  end-to-end here (no Chrome for Testing fetched in this environment). The HTTP
  contract is read from `engine.rs`; if a handler's shape differs at runtime,
  that's where it would surface. Recommend one smoke test against a live server
  before tagging.

## SySL disclosure

Per SySL-1.0 §2.3: portions of this crate were generated with the assistance of
an AI model (Anthropic Claude). Disclosed here and in the crate's `LICENSE`
copy. Human-authored the architecture, module boundaries, and all tests.

## If this isn't the shape you want

Three lighter alternatives, in case the scope is wrong:

1. **Don't merge into this repo** — keep `shirabe-crawler` as a separate repo
   that depends on shirabe published to crates.io. Zero changes to shirabe.
2. **Accept only the `PageDriver` trait** into shirabe proper, leave the rest as
   an external crate.
3. **Reject entirely** — close this PR. The design notes above are yours either
   way.
