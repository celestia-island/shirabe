//! shirabe — Lightweight headless browser automation.
//!
//! CDP (Chrome DevTools Protocol) engine + HTTP debug API.
//! Zero-config: finds Chrome automatically, launches headless, serves API.
//!
//! ## Quick Start
//!
//! ```no_run
//! use shirabe::{start_debug_server, DebugServerConfig};
//!
//! # async fn run() -> anyhow::Result<()> {
//! let cfg = DebugServerConfig {
//!     base_url: "about:blank".to_string(),
//!     dev_port: 0,
//!     dist_dir: String::new(),
//!     package_name: String::new(),
//!     proxy: None,
//! };
//! start_debug_server(cfg, 3001).await?;
//! # Ok(())
//! # }
//! ```
//!
//! Pick a browser backend with `SHIRABE_BACKEND=chrome|chromium|edge|auto`
//! (default `auto`) or pin one with `CHROME_PATH` / `CHROMIUM_PATH` /
//! `EDGE_PATH`. See the [`backend`](./backend/index.html) module for the full
//! resolution order.

pub mod backend;
pub mod browser_fetch;
pub mod bundle;
pub mod engine;
#[cfg(feature = "foreign-engine")]
pub mod ffi;
pub mod resolver;

pub use backend::{
    Backend, resolve as resolve_backend, resolve_executable as resolve_backend_executable,
};
pub use bundle::{
    BundleReport, BundleSpec, NativeLib, collect_runtime_deps, render as render_bundle_report,
};
pub use engine::{DebugServerConfig, start_debug_server};
#[cfg(feature = "foreign-engine")]
pub use ffi::{CdylibEngine, Engine};
pub use resolver::resolve_executable;
