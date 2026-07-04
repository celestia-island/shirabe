# 后端与解析

shirabe 通过单一 CDP 引擎驱动任何支持 Chrome DevTools 协议的浏览器 — Google
Chrome、Chromium、Microsoft Edge。使用 `SHIRABE_BACKEND` 选择：

| Value | Backend |
|-------|---------|
| `chrome` (default in `auto`) | Google Chrome |
| `chromium` | Chromium |
| `edge` | Microsoft Edge |
| `auto` (default) | Try Chrome, then Chromium, then Edge |

## 解析顺序

无论选择哪个后端，shirabe 按以下顺序解析可执行文件
（镜像 [ort](https://crates.io/crates/ort) 的依赖模型）：

1. **后端专属覆盖** — `CHROME_PATH` / `CHROMIUM_PATH` / `EDGE_PATH`。
   若已设置，该路径具有最高优先级；路径缺失则为硬错误。
2. **构建时嵌入路径** — `SHIRABE_BROWSER_PATH`，由 `build.rs` 生成，
   当 `auto-fetch` 功能在编译期间将固定版本的 Chrome for Testing 构建
   下载到共享缓存时设置。
3. **系统二进制文件** — `$PATH` 上的二进制文件，以及若干已知安装位置
   （`/usr/bin/google-chrome`、`/Applications/Google Chrome.app/...`、
   `C:\Program Files\Google\Chrome\Application\chrome.exe` 等）。
4. **运行时获取**（`runtime-fetch` 功能） — 首次使用时将固定版本的
   Chrome for Testing 构建下载到缓存中。

## 下载选项

获取步骤在构建时（`build.rs`）和运行时均会遵循以下环境变量：

| Env | Purpose |
|-----|---------|
| `SHIRABE_CHROME_VERSION` | 覆盖固定的 Chrome for Testing 版本。 |
| `SHIRABE_CHROME_MIRROR` | 从镜像（例如防火墙友好的镜像）下载，而不是默认的 Google 主机。 |
| `SHIRABE_CHROME_SHA256` | 可选的十六进制校验和；下载将根据此进行验证。 |
| `SHIRABE_DOWNLOAD_PROXY` | 通过 `http://`、`https://` 或 `socks5://` 代理路由下载。 |
| `SHIRABE_DOWNLOAD_TIMEOUT_SECS` | 每个请求的超时时间（默认 600）。 |
| `SHIRABE_SKIP_BROWSER_FETCH` | 跳过构建时和运行时的下载。 |

> 由于 `build.rs` 也会读取这些变量，下游 crate 可以在 CI 中通过单个
> `env:` 代码块固定整个工具链。
