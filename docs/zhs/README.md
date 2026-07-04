<p align="center"><img src="https://raw.githubusercontent.com/celestia-island/shirabe/master/docs/logo.webp" alt="shirabe" width="240" /></p>

<h1 align="center">shirabe</h1>

<p align="center"><strong>无头浏览器自动化</strong></p>

<div align="center">

[![License: SySL-1.0](https://img.shields.io/badge/License-SySL--1.0-blue.svg)](../../LICENSE)
[![Checks](https://img.shields.io/github/actions/workflow/status/celestia-island/shirabe/checks.yml)](https://github.com/celestia-island/shirabe/actions/workflows/checks.yml)
[![Docs](https://img.shields.io/badge/docs-shirabe.docs.celestia.world-blue)](https://shirabe.docs.celestia.world)

</div>

<div align="center">

[English](../en/README.md) ·
**简体中文** ·
[繁體中文](../zht/README.md) ·
[日本語](../ja/README.md) ·
[한국어](../ko/README.md) ·
[Français](../fr/README.md) ·
[Español](../es/README.md) ·
[Русский](../ru/README.md) ·
[العربية](../ar/README.md)

</div>

## 简介

shirabe 是一个轻量、Rust 原生的浏览器自动化库与调试服务器。它通过一套手写的
CDP 引擎驱动任何支持 Chrome DevTools Protocol 的浏览器——Google Chrome、Chromium、
Microsoft Edge——并通过一套精简的 HTTP API 对外暴露所有功能。它是从 tairitsu
打包器中剥离出来、经过独立强化的浏览器底座。

其设计理念与 ONNX Runtime 的 [ort](https://crates.io/crates/ort) 一致：**你永远不应该
手动安装浏览器。** 一份锁定的 Chrome for Testing 构建会在构建时（或首次使用时）
被拉取到共享缓存中，并透明地被定位和驱动。你可以锁定不同的后端、随产品分发原生库、
通过镜像或代理路由下载——全部通过环境变量完成。

## 快速开始

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

### 作为库使用

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

## 后端与零配置解析

通过 `SHIRABE_BACKEND=chrome|chromium|edge|firefox|servo|auto` 选择后端（默认为
`auto`）。**Chromium 家族**（Chrome / Chromium / Edge）通过我们自研的 CDP 引擎在
进程内驱动；**Firefox** 和 **Servo** 则走不同路径——其核心由浏览器厂商构建并以
动态库形式提供，shirabe 通过一个薄 C 绑定 FFI 合约来驱动（需要启用 `foreign-engine`
特性，详见 [Foreign Engines](../en/guides/foreign-engines.md)）。无论选择哪种后端，
shirabe 按以下顺序进行解析：

1. **后端专属覆盖** — `CHROME_PATH` / `CHROMIUM_PATH` / `EDGE_PATH`。
2. **构建时烘焙的路径** — `SHIRABE_BROWSER_PATH`，由 `build.rs` 在 `auto-fetch`
   特性启用时（构建阶段下载 Chrome for Testing）生成。
3. **系统二进制** — 在 `$PATH` 及常见安装路径中查找。
4. **运行时拉取**（需启用 `runtime-fetch` 特性）— 将锁定的构建下载到共享缓存。

下载相关配置选项（构建时与运行时均适用）：

| 环境变量 | 用途 |
|----------|------|
| `SHIRABE_CHROME_VERSION` | 覆盖锁定的 Chrome for Testing 版本。 |
| `SHIRABE_CHROME_MIRROR` | 从镜像站而非 `storage.googleapis.com` 下载。 |
| `SHIRABE_CHROME_SHA256` | 可选的十六进制校验和，用于验证下载。 |
| `SHIRABE_DOWNLOAD_PROXY` | 通过 `http://` / `https://` / `socks5://` 代理路由下载。 |
| `SHIRABE_DOWNLOAD_TIMEOUT_SECS` | 单次请求超时时间（默认 600 秒）。 |
| `SHIRABE_SKIP_BROWSER_FETCH` | 跳过构建时和运行时的浏览器下载。 |
| `SHIRABE_BACKEND` | 指定要驱动的 Chromium 家族后端。 |

## 随产品分发原生库

拉取的 Chrome 构建（以及你自己的 crate）依赖的原生库在干净的容器中可能缺失。
shirabe 为打包者提供了两个工具：

- **声明式打包** — 通过 `SHIRABE_BUNDLE_LIBS`（路径分隔符分隔的列表）或
  `SHIRABE_BUNDLE_MANIFEST`（一个包含 `[[lib]]` 表的 `bundle.toml` 文件）列出
  需要随附的 `.so` / `.dylib` / `.dll` 文件。示例 manifest：

  ```toml
  [[lib]]
  path = "third_party/libfoo.so"
  optional = true
  target_os = "linux"

  [[lib]]
  path = "third_party/foo.dll"
  ```

- **依赖扫描** — `shirabe::collect_runtime_deps(exe)` 会枚举二进制文件链接的
  共享库（`ldd` / `otool -L` / PE 导入扫描），而
  `shirabe::render_bundle_report(&BundleReport::build(&exe))` 会打印出发行脚本
  应随二进制文件一同复制的所有内容。

```rust
use shirabe::{BundleReport, render_bundle_report};
let report = BundleReport::build(&backend_exe);
print!("{}", render_bundle_report(&report));
```

## HTTP API

| 方法 | 路径 | 描述 |
|--------|------|-------------|
| `GET`  | `/health` | 服务器健康检查 |
| `GET`  | `/info` | 浏览器状态 + 所选后端 |
| `POST` | `/navigate` | 导航到指定 URL |
| `POST` | `/click` | 点击元素 |
| `POST` | `/type` | 输入文本 |
| `POST` | `/evaluate` | 执行 JavaScript |
| `POST` | `/screenshot` | 截取屏幕截图 |
| `POST` | `/wait-for-selector` | 等待元素出现 |
| `GET`  | `/dom` | 查询 DOM |
| `GET`  | `/a11y` | 无障碍树 |
| `POST` | `/batch` | 批量操作 |

……以及控制台、网络和 WebSocket 捕获端点，提供完整的控制能力。

## 开发

```bash
SHIRABE_SKIP_BROWSER_FETCH=1 cargo clippy --all-targets --all-features -- -D warnings
SHIRABE_SKIP_BROWSER_FETCH=1 cargo test --all-features
```

## 许可证

SySL-1.0（Synthetic Source License）。详见 [LICENSE](../../LICENSE)。
