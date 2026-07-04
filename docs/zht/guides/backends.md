# 後端與解析

shirabe 透過單一 CDP 引擎驅動任何支援 Chrome DevTools 協定的瀏覽器 — Google
Chrome、Chromium、Microsoft Edge。使用 `SHIRABE_BACKEND` 選擇：

| Value | Backend |
|-------|---------|
| `chrome` (default in `auto`) | Google Chrome |
| `chromium` | Chromium |
| `edge` | Microsoft Edge |
| `auto` (default) | Try Chrome, then Chromium, then Edge |

## 解析順序

無論選擇哪個後端，shirabe 按以下順序解析執行檔
（鏡像 [ort](https://crates.io/crates/ort) 的依賴模型）：

1. **後端專屬覆蓋** — `CHROME_PATH` / `CHROMIUM_PATH` / `EDGE_PATH`。
   若已設定，該路徑具有最高優先權；路徑缺失則為硬錯誤。
2. **建置時嵌入路徑** — `SHIRABE_BROWSER_PATH`，由 `build.rs` 產生，
   當 `auto-fetch` 功能在編譯期間將固定版本的 Chrome for Testing 建置
   下載到共享快取時設定。
3. **系統二進位檔案** — `$PATH` 上的二進位檔案，以及若干已知安裝位置
   （`/usr/bin/google-chrome`、`/Applications/Google Chrome.app/...`、
   `C:\Program Files\Google\Chrome\Application\chrome.exe` 等）。
4. **執行時取得**（`runtime-fetch` 功能） — 首次使用時將固定版本的
   Chrome for Testing 建置下載到快取中。

## 下載選項

取得步驟在建置時（`build.rs`）和執行時均會遵循以下環境變數：

| Env | Purpose |
|-----|---------|
| `SHIRABE_CHROME_VERSION` | 覆蓋固定的 Chrome for Testing 版本。 |
| `SHIRABE_CHROME_MIRROR` | 從鏡像（例如防火牆友好的鏡像）下載，而不是預設的 Google 主機。 |
| `SHIRABE_CHROME_SHA256` | 可選的十六進位校驗和；下載將根據此進行驗證。 |
| `SHIRABE_DOWNLOAD_PROXY` | 透過 `http://`、`https://` 或 `socks5://` 代理路由下載。 |
| `SHIRABE_DOWNLOAD_TIMEOUT_SECS` | 每個請求的逾時時間（預設 600）。 |
| `SHIRABE_SKIP_BROWSER_FETCH` | 跳過建置時和執行時的下載。 |

> 由於 `build.rs` 也會讀取這些變數，下游 crate 可以在 CI 中透過單一
> `env:` 區塊固定整個工具鏈。
