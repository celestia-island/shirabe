# Backends & Resolution

shirabe drives any browser that speaks the Chrome DevTools Protocol — Google
Chrome, Chromium, Microsoft Edge — through one CDP engine. Select one with
`SHIRABE_BACKEND`:

| Value | Backend |
|-------|---------|
| `chrome` (default in `auto`) | Google Chrome |
| `chromium` | Chromium |
| `edge` | Microsoft Edge |
| `auto` (default) | Try Chrome, then Chromium, then Edge |

## Resolution order

Whichever backend is chosen, shirabe resolves an executable in this order
(mirroring [ort](https://crates.io/crates/ort)'s dependency model):

1. **Backend-specific override** — `CHROME_PATH` / `CHROMIUM_PATH` / `EDGE_PATH`.
   If set, the path is authoritative; a missing path is a hard error.
2. **Build-time baked path** — `SHIRABE_BROWSER_PATH`, emitted by `build.rs`
   when the `auto-fetch` feature downloads the pinned Chrome for Testing build
   into the shared cache during compilation.
3. **System binary** on `$PATH` plus a handful of well-known install locations
   (`/usr/bin/google-chrome`, `/Applications/Google Chrome.app/...`,
   `C:\Program Files\Google\Chrome\Application\chrome.exe`, …).
4. **Runtime fetch** (the `runtime-fetch` feature) — download the pinned Chrome
   for Testing build on first use into the cache.

## Download knobs

The fetch step honours these environment variables, both at build time
(`build.rs`) and at runtime:

| Env | Purpose |
|-----|---------|
| `SHIRABE_CHROME_VERSION` | Override the pinned Chrome for Testing version. |
| `SHIRABE_CHROME_MIRROR` | Download from a mirror (e.g. a GFW-friendly one) instead of the default Google host. |
| `SHIRABE_CHROME_SHA256` | Optional hex checksum; the download is verified against it. |
| `SHIRABE_DOWNLOAD_PROXY` | Route the download through an `http://`, `https://` or `socks5://` proxy. |
| `SHIRABE_DOWNLOAD_TIMEOUT_SECS` | Per-request timeout (default 600). |
| `SHIRABE_SKIP_BROWSER_FETCH` | Skip both build-time and runtime downloads. |

> Because `build.rs` reads these too, a downstream crate can pin the whole
> toolchain in CI with one `env:` block.
