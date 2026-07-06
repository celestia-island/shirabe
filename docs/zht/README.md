<p align="center"><img src="https://raw.githubusercontent.com/celestia-island/shirabe/master/docs/logo.webp" alt="shirabe" width="240" /></p>

<h1 align="center">shirabe</h1>

<p align="center"><strong>無頭瀏覽器自動化</strong></p>

<div align="center">

[![License: SySL-1.0](https://img.shields.io/badge/License-SySL--1.0-blue.svg)](https://sysl.celestia.world)
[![Checks](https://img.shields.io/github/actions/workflow/status/celestia-island/shirabe/checks.yml)](https://github.com/celestia-island/shirabe/actions/workflows/checks.yml)
[![Docs](https://img.shields.io/badge/docs-shirabe.docs.celestia.world-blue)](https://shirabe.docs.celestia.world)

</div>

<div align="center">

[English](../en/README.md) ·
[简体中文](../zhs/README.md) ·
**繁體中文** ·
[日本語](../ja/README.md) ·
[한국어](../ko/README.md) ·
[Français](../fr/README.md) ·
[Español](../es/README.md) ·
[Русский](../ru/README.md) ·
[العربية](../ar/README.md)

</div>

## 簡介

shirabe 是一個輕量級、Rust 原生的瀏覽器自動化函式庫與除錯伺服器。它透過一套自行實
作的 CDP 引擎，驅動任何支援 Chrome DevTools Protocol 的瀏覽器——Google Chrome、
Chromium、Microsoft Edge——並以一套精簡的 HTTP API 對外暴露所有功能。它是從
tairitsu 打包器中剝離出來的瀏覽器底層，經過強化以獨立運行。

其核心設計理念與 ONNX Runtime 的 [ort](https://crates.io/crates/ort) 一致：**你永遠
不需要手動安裝瀏覽器。** 一份鎖定版本的 Chrome for Testing 建置會在編譯期間（或首次
使用時）被拉取到共享快取中，透明地定位後透過 CDP 驅動。你也可以鎖定不同的後端、將
原生函式庫隨產品一起發布、透過鏡像站或代理伺服器進行下載——全部透過環境變數設定。

## 快速開始

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

### 函式庫

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

## 後端與零設定解析

透過 `SHIRABE_BACKEND=chrome|chromium|edge|firefox|servo|auto` 選擇後端（預設為
`auto`）。**Chromium 家族**（Chrome / Chromium / Edge）由我們自己的 CDP 引擎在程序
內直接驅動；**Firefox** 和 **Servo** 則走不同路徑——其核心由瀏覽器廠商建置並以動態
函式庫形式發布，shirabe 透過一層精簡的 C 繫結 FFI 合約來驅動（`foreign-engine`
功能，詳見[外部引擎](../en/guides/foreign-engines.md)）。無論選擇哪一種，
shirabe 的解析順序如下：

1. **後端專屬覆寫**——`CHROME_PATH` / `CHROMIUM_PATH` / `EDGE_PATH`。
2. **編譯期內嵌路徑**——`SHIRABE_BROWSER_PATH`，當 `auto-fetch` 功能在編譯期間
   下載 Chrome for Testing 時，由 `build.rs` 產生。
3. **系統二進位檔**——搜尋 `$PATH` 及已知的常見安裝位置。
4. **執行期下載**（`runtime-fetch` 功能）——將鎖定版本的建置下載到共享快取中。

下載參數（編譯期與執行期均適用）：

| 環境變數 | 用途 |
|-----|---------|
| `SHIRABE_CHROME_VERSION` | 覆寫鎖定的 Chrome for Testing 版本。 |
| `SHIRABE_CHROME_MIRROR` | 從鏡像站下載，而非 `storage.googleapis.com`。 |
| `SHIRABE_CHROME_SHA256` | 可選的十六進位校驗碼，用於驗證下載檔案。 |
| `SHIRABE_DOWNLOAD_PROXY` | 透過 `http://` / `https://` / `socks5://` 代理伺服器進行下載。 |
| `SHIRABE_DOWNLOAD_TIMEOUT_SECS` | 每個請求的逾時時間（預設 600 秒）。 |
| `SHIRABE_SKIP_BROWSER_FETCH` | 同時跳過編譯期和執行期的瀏覽器下載。 |
| `SHIRABE_BACKEND` | 指定要驅動的 Chromium 家族後端。 |

## 將原生函式庫隨產品一起發布

拉取下來的 Chrome 建置（以及你自己的 crate）可能依賴一些在乾淨容器中不存在的原生
函式庫。shirabe 為打包者提供了兩種工具：

- **宣告式捆綁**——透過 `SHIRABE_BUNDLE_LIBS`（路徑分隔符號清單）或
  `SHIRABE_BUNDLE_MANIFEST`（一個包含 `[[lib]]` 表格的 `bundle.toml`）列出要隨附
  的 `.so` / `.dylib` / `.dll` 檔案。範例清單：

  ```toml
  [[lib]]
  path = "third_party/libfoo.so"
  optional = true
  target_os = "linux"

  [[lib]]
  path = "third_party/foo.dll"
  ```

- **依賴掃描**——`shirabe::collect_runtime_deps(exe)` 會列舉一個二進位檔所鏈結的
  共用函式庫（`ldd` / `otool -L` / PE 匯入掃描），而
  `shirabe::render_bundle_report(&BundleReport::build(&exe))` 則會印出發行腳本應
  隨二進位檔一併複製的所有內容。

```rust
use shirabe::{BundleReport, render_bundle_report};
let report = BundleReport::build(&backend_exe);
print!("{}", render_bundle_report(&report));
```

## HTTP API

| 方法 | 路徑 | 說明 |
|--------|------|-------------|
| `GET`  | `/health` | 伺服器健康狀態 |
| `GET`  | `/info` | 瀏覽器狀態與選定的後端 |
| `POST` | `/navigate` | 導覽至指定 URL |
| `POST` | `/click` | 點擊元素 |
| `POST` | `/type` | 輸入文字 |
| `POST` | `/evaluate` | 執行 JavaScript |
| `POST` | `/screenshot` | 擷取螢幕截圖 |
| `POST` | `/wait-for-selector` | 等待元素出現 |
| `GET`  | `/dom` | 查詢 DOM |
| `GET`  | `/a11y` | 輔助功能樹 |
| `POST` | `/batch` | 批次操作 |

……以及控制台、網路和 WebSocket 擷取端點，提供完整控制能力。

## 開發

```bash
SHIRABE_SKIP_BROWSER_FETCH=1 cargo clippy --all-targets --all-features -- -D warnings
SHIRABE_SKIP_BROWSER_FETCH=1 cargo test --all-features
```

## 授權條款

SySL-1.0（Synthetic Source License）。詳見 [LICENSE](https://sysl.celestia.world)。
