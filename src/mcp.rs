//! Standalone MCP (Model Context Protocol) server for shirabe.
//!
//! Hosts shirabe's own headless-browser debug HTTP API in-process on an
//! ephemeral loopback port, then exposes its operations as MCP tools over
//! stdio. So an AI coding assistant drives a real headless Chromium — navigate,
//! click, type, evaluate, screenshot — with no separate `shirabe debug` daemon
//! to launch: the one `shirabe mcp` process is both the CDP engine and the MCP
//! server.
//!
//! This is the shirabe half of what `tairitsu-mcp` shipped, but in-process
//! rather than proxying to an external daemon.
//!
//! # Usage
//!
//! ```ignore
//! shirabe mcp
//! ```

#![cfg(feature = "mcp")]

use anyhow::Result;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    handler::server::wrapper::Parameters, model::*, service::RequestContext, tool, tool_handler,
    tool_router,
};
use schemars::JsonSchema;

use crate::{DebugServerConfig, start_debug_server};

struct Server {
    base_url: Arc<RwLock<String>>,
    http: reqwest::Client,
}

impl Server {
    async fn api(&self, path: &str) -> String {
        let base = self.base_url.read().await.clone();
        format!("{base}/{path}")
    }

    async fn ensure_up(&self) -> Result<String, McpError> {
        let url = self.base_url.read().await.clone();
        if url.is_empty() {
            return Err(McpError::internal_error(
                "Debug server is not up yet. Wait a moment and retry.",
                None,
            ));
        }
        Ok(url)
    }

    fn tool_result(text: impl Into<String>) -> CallToolResult {
        CallToolResult::success(vec![Content::text(text)])
    }

    async fn http_post(&self, path: &str, body: Value) -> Result<Value, McpError> {
        let url = self.api(path).await;
        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| McpError::internal_error(format!("HTTP request failed: {e}"), None))?;
        let status = resp.status();
        let v: Value = resp
            .json()
            .await
            .map_err(|e| McpError::internal_error(format!("Bad response body: {e}"), None))?;
        if !status.is_success() {
            let msg = v
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("unknown error");
            return Err(McpError::internal_error(
                format!("debug server returned {status}: {msg}"),
                None,
            ));
        }
        Ok(v)
    }

    async fn http_get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value, McpError> {
        let base = self.api(path).await;
        // Build the query string manually: shirabe's reqwest is built without the
        // `url`/query feature to stay dependency-light.
        let url = if query.is_empty() {
            base
        } else {
            let qs = query
                .iter()
                .map(|(k, v)| format!("{}={}", url_encode(k), url_encode(v)))
                .collect::<Vec<_>>()
                .join("&");
            format!("{base}?{qs}")
        };
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| McpError::internal_error(format!("HTTP request failed: {e}"), None))?;
        let status = resp.status();
        let v: Value = resp
            .json()
            .await
            .map_err(|e| McpError::internal_error(format!("Bad response body: {e}"), None))?;
        if !status.is_success() {
            let msg = v
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("unknown error");
            return Err(McpError::internal_error(
                format!("debug server returned {status}: {msg}"),
                None,
            ));
        }
        Ok(v)
    }

    async fn http_post_fire_and_forget(&self, path: &str, body: Value) -> Result<(), McpError> {
        let url = self.api(path).await;
        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| McpError::internal_error(format!("HTTP request failed: {e}"), None))?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(McpError::internal_error(
                format!("debug server returned {status}: {text}"),
                None,
            ));
        }
        Ok(())
    }
}

// ── Tool argument structs ────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
struct NavigateArgs {
    url: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SnapshotArgs {
    /// Optional CSS selector to scope the snapshot to a subtree.
    target: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct DomQueryArgs {
    selector: String,
    attribute: Option<String>,
    #[serde(rename = "computed")]
    computed: Option<bool>,
    #[serde(rename = "all")]
    all: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ScreenshotArgs {
    element: Option<String>,
    #[serde(rename = "fullPage")]
    full_page: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ClickArgs {
    target: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct TypeArgs {
    target: String,
    text: String,
    submit: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct PressKeyArgs {
    key: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct EvaluateArgs {
    function: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ConsoleMessagesArgs {
    /// Minimum log level: error, warning, info, debug.
    level: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ResizeArgs {
    width: u32,
    height: u32,
}

// ── Browser tools (HTTP proxy to the in-process debug server) ─────────

#[tool_router]
impl Server {
    #[tool(description = "Navigate to a URL")]
    async fn browser_navigate(
        &self,
        Parameters(args): Parameters<NavigateArgs>,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_up().await?;
        self.http_post_fire_and_forget("navigate", json!({"url": args.url}))
            .await?;
        Ok(Self::tool_result(format!("Navigated to {}", args.url)))
    }

    #[tool(description = "Go back to the previous page")]
    async fn browser_navigate_back(
        &self,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_up().await?;
        self.http_post_fire_and_forget("back", json!({})).await?;
        Ok(Self::tool_result("Navigated back"))
    }

    #[tool(description = "Go forward to the next page")]
    async fn browser_navigate_forward(
        &self,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_up().await?;
        self.http_post_fire_and_forget("forward", json!({})).await?;
        Ok(Self::tool_result("Navigated forward"))
    }

    #[tool(
        description = "Capture accessibility snapshot of the current page (DOM tree with roles, names, text). Better than screenshot for understanding page structure."
    )]
    async fn browser_snapshot(
        &self,
        Parameters(args): Parameters<SnapshotArgs>,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_up().await?;
        let query: Vec<(&str, &str)> = args
            .target
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|s| vec![("selector", s)])
            .unwrap_or_default();
        let v = self.http_get("a11y", &query).await?;
        Ok(Self::tool_result(
            v.get("data")
                .map(|d| serde_json::to_string(d).unwrap_or_else(|_| "{}".into()))
                .unwrap_or_else(|| "{}".into()),
        ))
    }

    #[tool(
        description = "Query a DOM element by CSS selector — returns its tag, text, html, attributes, visibility, bounding rect, and match count. Pass `attribute` to fetch just one attribute's value. Complements `browser_snapshot` (semantic a11y tree) with exact element details."
    )]
    async fn browser_dom(
        &self,
        Parameters(args): Parameters<DomQueryArgs>,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_up().await?;
        let mut query: Vec<(&str, &str)> = vec![("selector", args.selector.as_str())];
        if let Some(attr) = args.attribute.as_deref() {
            if !attr.is_empty() {
                query.push(("attribute", attr));
            }
        }
        if matches!(args.computed, Some(true)) {
            query.push(("computed", "true"));
        }
        if matches!(args.all, Some(true)) {
            query.push(("all", "true"));
        }
        let v = self.http_get("dom", &query).await?;
        Ok(Self::tool_result(
            v.get("data")
                .map(|d| serde_json::to_string(d).unwrap_or_else(|_| "{}".into()))
                .unwrap_or_else(|| "{}".into()),
        ))
    }

    #[tool(
        description = "Take a screenshot of the current viewport as PNG (returns base64 data URL)"
    )]
    async fn browser_screenshot(
        &self,
        Parameters(args): Parameters<ScreenshotArgs>,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_up().await?;
        let mut body = json!({});
        if let Some(el) = &args.element {
            body["selector"] = json!(el);
        }
        if let Some(fp) = args.full_page {
            body["full_page"] = json!(fp);
        }
        let v = self.http_post("screenshot", body).await?;
        let ok = v.get("ok").and_then(|s| s.as_bool()).unwrap_or(false);
        if ok {
            let data = v
                .get("data")
                .and_then(|d| {
                    d.as_str()
                        .map(|s| s.to_string())
                        .or_else(|| {
                            d.get("data")
                                .and_then(|dd| dd.as_str())
                                .map(|s| s.to_string())
                        })
                        .or_else(|| {
                            d.as_object()
                                .map(|_| serde_json::to_string(d).unwrap_or_default())
                        })
                })
                .unwrap_or_default();
            let mime = v
                .get("data")
                .and_then(|d| d.get("mime_type"))
                .and_then(|m| m.as_str())
                .unwrap_or("image/png");
            let data_url = if data.starts_with("data:") {
                data
            } else {
                format!("data:{mime};base64,{data}")
            };
            Ok(CallToolResult::success(vec![Content::text(data_url)]))
        } else {
            let err = v
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("unknown")
                .to_string();
            Err(McpError::internal_error(err, None))
        }
    }

    #[tool(description = "Click an element by CSS selector or reference from snapshot")]
    async fn browser_click(
        &self,
        Parameters(args): Parameters<ClickArgs>,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_up().await?;
        self.http_post_fire_and_forget("click", json!({"selector": args.target}))
            .await?;
        Ok(Self::tool_result(format!("Clicked: {}", args.target)))
    }

    #[tool(description = "Type text into an editable element (input, textarea, contenteditable)")]
    async fn browser_type(
        &self,
        Parameters(args): Parameters<TypeArgs>,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_up().await?;
        self.http_post_fire_and_forget(
            "type",
            json!({
                "selector": args.target,
                "text": args.text,
                "clear_first": false,
                "submit": args.submit.unwrap_or(false)
            }),
        )
        .await?;
        Ok(Self::tool_result(format!("Typed: {}", args.text)))
    }

    #[tool(description = "Press a keyboard key (Enter, Tab, Escape, ArrowUp, etc.)")]
    async fn browser_press_key(
        &self,
        Parameters(args): Parameters<PressKeyArgs>,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_up().await?;
        self.http_post_fire_and_forget("press", json!({"key": args.key}))
            .await?;
        Ok(Self::tool_result(format!("Pressed: {}", args.key)))
    }

    #[tool(description = "Evaluate JavaScript expression in the page context and return result")]
    async fn browser_evaluate(
        &self,
        Parameters(args): Parameters<EvaluateArgs>,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_up().await?;
        let v = self
            .http_post("evaluate", json!({"expression": args.function}))
            .await?;
        Ok(Self::tool_result(
            serde_json::to_string_pretty(&v.get("data").unwrap_or(&v)).unwrap_or_default(),
        ))
    }

    #[tool(description = "Get console log entries (error/warning/info/debug) from the page")]
    async fn browser_console_messages(
        &self,
        Parameters(args): Parameters<ConsoleMessagesArgs>,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_up().await?;
        let query: Vec<(&str, &str)> = args
            .level
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|l| vec![("level", l)])
            .unwrap_or_default();
        let v = self.http_get("console", &query).await?;
        Ok(Self::tool_result(
            serde_json::to_string_pretty(&v.get("data").unwrap_or(&v)).unwrap_or_default(),
        ))
    }

    #[tool(description = "Resize the browser viewport")]
    async fn browser_resize(
        &self,
        Parameters(args): Parameters<ResizeArgs>,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        self.ensure_up().await?;
        self.http_post_fire_and_forget(
            "resize",
            json!({"width": args.width, "height": args.height}),
        )
        .await?;
        Ok(Self::tool_result(format!(
            "Resized to {}x{}",
            args.width, args.height
        )))
    }
}

// ── ServerHandler ────────────────────────────────────

#[tool_handler(router = Server::tool_router())]
impl ServerHandler for Server {}

// ── helpers ──────────────────────────────────────────

/// Minimal percent-encoding for a query string value (the selectors/keys we
/// pass are ASCII CSS selectors + short strings; this covers the necessary set
/// without pulling in a `url`/percent-encoding crate).
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Grab a free loopback TCP port by binding to :0 then dropping the listener.
/// There is a tiny TOCTOU window before `start_debug_server` rebinds it, but
/// for a single-shot local process launch it is negligible.
fn free_loopback_port() -> Result<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

/// Resolve the initial page URL for the browser: an explicit `SHIRABE_URL`
/// override, otherwise about:blank.
fn initial_base_url() -> String {
    std::env::var("SHIRABE_URL").unwrap_or_else(|_| "about:blank".to_string())
}

// ── public entry point ───────────────────────────────

pub async fn run() -> Result<()> {
    // Install the rustls crypto provider once, before any reqwest::Client is
    // built (the debug server + browser download both use rustls-no-provider).
    let _ = rustls::crypto::ring::default_provider().install_default();

    let base_url = Arc::new(RwLock::new(String::new()));

    // Spawn the in-process headless-browser debug server on an ephemeral port.
    // Its lifetime is tied to this process; when the MCP server exits, so does
    // the browser (Chrome is killed on drop via kill_on_drop in spawn_browser).
    let port = free_loopback_port()?;
    let debug_cfg = DebugServerConfig {
        base_url: initial_base_url(),
        dev_port: 0,
        dist_dir: String::new(),
        package_name: String::new(),
        proxy: std::env::var("SHIRABE_DOWNLOAD_PROXY")
            .ok()
            .filter(|s| !s.is_empty()),
    };
    let base_url_clone = Arc::clone(&base_url);
    tokio::spawn(async move {
        let target = format!("http://127.0.0.1:{port}");
        // Wait for the debug server's /health to come up, then publish its URL
        // so the MCP tools can start proxying.
        let probe = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap_or_default();
        // start_debug_server blocks until cancelled; run it in the foreground of
        // this task. First, hand off the health-check to a short poller that
        // flips base_url once the server answers.
        let published = Arc::clone(&base_url_clone);
        tokio::spawn(async move {
            let deadline = std::time::Instant::now() + Duration::from_secs(45);
            while std::time::Instant::now() < deadline {
                if probe
                    .get(format!("{target}/health"))
                    .send()
                    .await
                    .is_ok_and(|r| r.status().is_success())
                {
                    *published.write().await = target;
                    return;
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            tracing::warn!(
                "debug server never became healthy within 45s; browser tools will error"
            );
        });
        if let Err(e) = start_debug_server(debug_cfg, port).await {
            tracing::error!(error = %e, "in-process debug server exited");
        }
    });

    let server = Server {
        base_url,
        http: reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default(),
    };

    let transport = rmcp::transport::stdio();
    let server_handle = server.serve(transport).await?;
    server_handle.waiting().await?;

    Ok(())
}
