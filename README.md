<p align="center"><img src="https://raw.githubusercontent.com/celestia-island/shirabe/master/docs/logo.webp" alt="shirabe" width="240" /></p>

<h1 align="center">shirabe</h1>

<p align="center"><strong>Headless browser automation</strong></p>

<div align="center">

[![License: SySL-1.0](https://img.shields.io/badge/License-SySL--1.0-blue.svg)](https://sysl.celestia.world)
[![Checks](https://img.shields.io/github/actions/workflow/status/celestia-island/shirabe/checks.yml)](https://github.com/celestia-island/shirabe/actions/workflows/checks.yml)
[![Docs](https://img.shields.io/badge/docs-shirabe.docs.celestia.world-blue)](https://shirabe.docs.celestia.world)

</div>

<div align="center">

**English** ·
[简体中文](./docs/zhs/README.md) ·
[繁體中文](./docs/zht/README.md) ·
[日本語](./docs/ja/README.md) ·
[한국어](./docs/ko/README.md) ·
[Français](./docs/fr/README.md) ·
[Español](./docs/es/README.md) ·
[Русский](./docs/ru/README.md) ·
[العربية](./docs/ar/README.md)

</div>

## Introduction

shirabe is a lightweight, Rust-native browser automation library and debug
server. It drives any browser that speaks the Chrome DevTools Protocol — Google
Chrome, Chromium, Microsoft Edge — through one hand-rolled CDP engine, and
exposes the whole thing over a small HTTP API. It is the browser backbone
extracted from the tairitsu packager, hardened to stand on its own.

The guiding idea is the same as [ort](https://crates.io/crates/ort) for ONNX
Runtime: **you should never have to install a browser by hand.** A pinned
Chrome for Testing build is fetched into a shared cache at build time (or on
first use), located transparently, and driven through CDP. Pin a different
backend, ship native libs with your product, route the download through a
mirror or proxy — all from environment variables.

## Quick Start

### CLI

```bash
# Zero-config: auto-discovers Chrome/Chromium/Edge, or fetches Chrome for Testing.
shirabe debug --port 3001

# Pin a backend, route the browser through a proxy.
SHIRABE_BACKEND=chromium shirabe debug --port 3001 --proxy http://localhost:7890

# Then drive it over HTTP.
curl -X POST http://localhost:3001/navigate \
  -H "Content-Type: application/json" -d '{"url":"https://example.com"}'
curl -X POST http://localhost:3001/screenshot -d '{}'
```

### Library

```rust
use shirabe::{start_debug_server, DebugServerConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = DebugServerConfig {
        base_url: "about:blank".to_string(),
        dev_port: 0,
        dist_dir: String::new(),
        package_name: String::new(),
        proxy: Some("http://localhost:7890".to_string()),
    };
    start_debug_server(cfg, 3001).await
}
```

## Backends & zero-config resolution

Pick a backend with `SHIRABE_BACKEND=chrome|chromium|edge|firefox|servo|auto`
(default `auto`). The **Chromium family** (Chrome / Chromium / Edge) is driven
in-process through our own CDP engine; **Firefox** and **Servo** take a
different path — their cores are built by the browser vendors and shipped as
dynamic libraries, which shirabe drives through a thin C-binding FFI contract
(the `foreign-engine` feature, see
[Foreign Engines](./docs/en/guides/foreign-engines.md)). Whichever is chosen,
shirabe resolves it in this order:

1. **Backend-specific override** — `CHROME_PATH` / `CHROMIUM_PATH` / `EDGE_PATH`.
2. **Build-time baked path** — `SHIRABE_BROWSER_PATH`, emitted by `build.rs`
   when the `auto-fetch` feature downloads Chrome for Testing during the build.
3. **System binary** on `$PATH` and well-known install locations.
4. **Runtime fetch** (the `runtime-fetch` feature) — download the pinned build
   into the shared cache.

Download knobs (build time and runtime alike):

| Env | Purpose |
|-----|---------|
| `SHIRABE_CHROME_VERSION` | Override the pinned Chrome for Testing version. |
| `SHIRABE_CHROME_MIRROR` | Download from a mirror instead of `storage.googleapis.com`. |
| `SHIRABE_CHROME_SHA256` | Optional hex checksum to verify the download. |
| `SHIRABE_DOWNLOAD_PROXY` | Route the download through `http://` / `https://` / `socks5://` proxy. |
| `SHIRABE_DOWNLOAD_TIMEOUT_SECS` | Per-request timeout (default 600). |
| `SHIRABE_SKIP_BROWSER_FETCH` | Skip both build-time and runtime downloads. |
| `SHIRABE_BACKEND` | Which Chromium-family backend to drive. |

## Shipping native libraries with your product

A fetched Chrome build (and your own crate) depend on native libraries that a
clean container may lack. shirabe gives packagers two tools:

- **Declarative bundle** — list the `.so` / `.dylib` / `.dll` files to ship via
  `SHIRABE_BUNDLE_LIBS` (path-sep list) or `SHIRABE_BUNDLE_MANIFEST` (a
  `bundle.toml` of `[[lib]]` tables). Example manifest:

  ```toml
  [[lib]]
  path = "third_party/libfoo.so"
  optional = true
  target_os = "linux"

  [[lib]]
  path = "third_party/foo.dll"
  ```

- **Dependency scan** — `shirabe::collect_runtime_deps(exe)` enumerates the
  shared libraries a binary links against (`ldd` / `otool -L` / a PE import
  scan), and `shirabe::render_bundle_report(&BundleReport::build(&exe))` prints
  everything a release script should copy alongside the binary.

```rust
use shirabe::{BundleReport, render_bundle_report};
let report = BundleReport::build(&backend_exe);
print!("{}", render_bundle_report(&report));
```

## HTTP API

| Method | Path | Description |
|--------|------|-------------|
| `GET`  | `/health` | Server health |
| `GET`  | `/info` | Browser status + selected backend |
| `POST` | `/navigate` | Navigate to a URL |
| `POST` | `/click` | Click an element |
| `POST` | `/type` | Type text |
| `POST` | `/evaluate` | Execute JavaScript |
| `POST` | `/screenshot` | Capture a screenshot |
| `POST` | `/wait-for-selector` | Wait for an element |
| `GET`  | `/dom` | Query the DOM |
| `GET`  | `/a11y` | Accessibility tree |
| `POST` | `/batch` | Batch operations |

…plus console, network and websocket capture endpoints for full control.

## Development

```bash
SHIRABE_SKIP_BROWSER_FETCH=1 cargo clippy --all-targets --all-features -- -D warnings
SHIRABE_SKIP_BROWSER_FETCH=1 cargo test --all-features
```


<details>
<summary>Screenshots</summary>

<p align="center"><img src="res/debug_server_solarized_dark.png" alt="shirabe snapshot" height="400" /></p>

</details>

## License

SySL-1.0 (Synthetic Source License). See [LICENSE](https://sysl.celestia.world).
