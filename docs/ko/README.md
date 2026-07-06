<p align="center"><img src="https://raw.githubusercontent.com/celestia-island/shirabe/master/docs/logo.webp" alt="shirabe" width="240" /></p>

<h1 align="center">shirabe</h1>

<p align="center"><strong>헤드리스 브라우저 자동화</strong></p>

<div align="center">

[![License: SySL-1.0](https://img.shields.io/badge/License-SySL--1.0-blue.svg)](https://sysl.celestia.world)
[![Checks](https://img.shields.io/github/actions/workflow/status/celestia-island/shirabe/checks.yml)](https://github.com/celestia-island/shirabe/actions/workflows/checks.yml)
[![Docs](https://img.shields.io/badge/docs-shirabe.docs.celestia.world-blue)](https://shirabe.docs.celestia.world)

</div>

<div align="center">

[English](../en/README.md) ·
[简体中文](../zhs/README.md) ·
[繁體中文](../zht/README.md) ·
[日本語](../ja/README.md) ·
**한국어** ·
[Français](../fr/README.md) ·
[Español](../es/README.md) ·
[Русский](../ru/README.md) ·
[العربية](../ar/README.md)

</div>

## 소개

shirabe는 가볍고 Rust 네이티브인 브라우저 자동화 라이브러리이자 디버그 서버입니다.
Chrome DevTools Protocol을 지원하는 모든 브라우저(Google Chrome·Chromium·Microsoft
Edge)를 직접 제작한 CDP 엔진으로 구동하며, 이를 작은 HTTP API로 외부에 제공합니다.
tairitsu 패키저에서 브라우저 계층을 분리하여 독립적으로 사용할 수 있도록 다듬은
프로젝트입니다.

기본 철학은 ONNX Runtime을 위한 [ort](https://crates.io/crates/ort)와 동일합니다:
**브라우저를 직접 설치할 필요가 전혀 없어야 합니다.** 고정된 Chrome for Testing
빌드가 빌드 시(또는 최초 사용 시) 공유 캐시로 내려받아지고, 투명하게 위치를
찾아내어 CDP를 통해 구동됩니다. 환경 변수만으로 다른 백엔드를 지정하거나, 제품에
네이티브 라이브러리를 동봉하거나, 다운로드를 미러나 프록시로 경유시킬 수 있습니다.

## 빠른 시작

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

### 라이브러리

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

## 백엔드 & 무설정 리졸버

`SHIRABE_BACKEND=chrome|chromium|edge|firefox|servo|auto`(기본값 `auto`)로 백엔드를
선택합니다. **Chromium 계열**(Chrome / Chromium / Edge)은 자체 CDP 엔진으로 프로세스
내에서 구동되며, **Firefox**와 **Servo**는 다른 경로를 따릅니다 — 브라우저 벤더가
빌드한 코어가 동적 라이브러리로 제공되고, shirabe는 얇은 C-바인딩 FFI 계약을 통해
이를 구동합니다(`foreign-engine` 기능, [외부 엔진](../en/guides/foreign-engines.md)
참조). 어떤 백엔드를 선택하든 shirabe는 다음 순서로 해결합니다:

1. **백엔드 전용 재정의** — `CHROME_PATH` / `CHROMIUM_PATH` / `EDGE_PATH`.
2. **빌드 시 고정 경로** — `SHIRABE_BROWSER_PATH`. `auto-fetch` 기능이 빌드 중에
   Chrome for Testing을 다운로드할 때 `build.rs`가 내보냅니다.
3. **시스템 바이너리** — `$PATH` 및 알려진 설치 위치에서 탐색.
4. **런타임 내려받기** (`runtime-fetch` 기능) — 고정된 빌드를 공유 캐시로 다운로드합니다.

다운로드 제어 변수 (빌드 시 및 런타임 공통):

| 환경 변수 | 용도 |
|-----|---------|
| `SHIRABE_CHROME_VERSION` | 고정된 Chrome for Testing 버전을 재정의합니다. |
| `SHIRABE_CHROME_MIRROR` | `storage.googleapis.com` 대신 미러에서 다운로드합니다. |
| `SHIRABE_CHROME_SHA256` | 다운로드 검증용 hex 체크섬 (선택 사항). |
| `SHIRABE_DOWNLOAD_PROXY` | `http://` / `https://` / `socks5://` 프록시로 다운로드를 경유합니다. |
| `SHIRABE_DOWNLOAD_TIMEOUT_SECS` | 요청별 타임아웃 (기본값 600). |
| `SHIRABE_SKIP_BROWSER_FETCH` | 빌드 시 및 런타임 다운로드를 모두 건너뜁니다. |
| `SHIRABE_BACKEND` | 구동할 Chromium 계열 백엔드를 지정합니다. |

## 제품에 네이티브 라이브러리 동봉하기

내려받은 Chrome 빌드(와 여러분의 크레이트)는 깨끗한 컨테이너에 없을 수 있는
네이티브 라이브러리에 의존합니다. shirabe는 패키저를 위한 두 가지 도구를 제공합니다:

- **선언적 번들** — `SHIRABE_BUNDLE_LIBS`(경로 구분자 목록) 또는
  `SHIRABE_BUNDLE_MANIFEST`(`[[lib]]` 테이블로 구성된 `bundle.toml`)를 통해
  동봉할 `.so` / `.dylib` / `.dll` 파일을 나열합니다. 매니페스트 예시:

  ```toml
  [[lib]]
  path = "third_party/libfoo.so"
  optional = true
  target_os = "linux"

  [[lib]]
  path = "third_party/foo.dll"
  ```

- **의존성 스캔** — `shirabe::collect_runtime_deps(exe)`는 바이너리가 링크하는
  공유 라이브러리를 열거하고(`ldd` / `otool -L` / PE 임포트 스캔),
  `shirabe::render_bundle_report(&BundleReport::build(&exe))`는 릴리스 스크립트가
  바이너리와 함께 복사해야 할 모든 항목을 출력합니다.

```rust
use shirabe::{BundleReport, render_bundle_report};
let report = BundleReport::build(&backend_exe);
print!("{}", render_bundle_report(&report));
```

## HTTP API

| 메서드 | 경로 | 설명 |
|--------|------|-------------|
| `GET`  | `/health` | 서버 상태 |
| `GET`  | `/info` | 브라우저 상태 + 선택된 백엔드 |
| `POST` | `/navigate` | URL로 이동 |
| `POST` | `/click` | 요소 클릭 |
| `POST` | `/type` | 텍스트 입력 |
| `POST` | `/evaluate` | JavaScript 실행 |
| `POST` | `/screenshot` | 스크린샷 캡처 |
| `POST` | `/wait-for-selector` | 요소 대기 |
| `GET`  | `/dom` | DOM 조회 |
| `GET`  | `/a11y` | 접근성 트리 |
| `POST` | `/batch` | 배치 작업 |

…이 외에도 콘솔, 네트워크, 웹소켓 캡처 엔드포인트를 통해 완전한 제어가 가능합니다.

## 개발

```bash
SHIRABE_SKIP_BROWSER_FETCH=1 cargo clippy --all-targets --all-features -- -D warnings
SHIRABE_SKIP_BROWSER_FETCH=1 cargo test --all-features
```

## 라이선스

SySL-1.0 (Synthetic Source License). [LICENSE](https://sysl.celestia.world) 참조.
