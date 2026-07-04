# 백엔드 및 해석

shirabe는 Chrome DevTools 프로토콜을 사용하는 모든 브라우저 — Google Chrome,
Chromium, Microsoft Edge — 를 단일 CDP 엔진으로 구동합니다. `SHIRABE_BACKEND`로
선택하세요:

| Value | Backend |
|-------|---------|
| `chrome` (default in `auto`) | Google Chrome |
| `chromium` | Chromium |
| `edge` | Microsoft Edge |
| `auto` (default) | Try Chrome, then Chromium, then Edge |

## 해석 순서

어떤 백엔드가 선택되든, shirabe는 다음 순서로 실행 파일을 해석합니다
([ort](https://crates.io/crates/ort)의 의존성 모델을 반영):

1. **백엔드별 재정의** — `CHROME_PATH` / `CHROMIUM_PATH` / `EDGE_PATH`.
   설정된 경우 해당 경로가 우선됩니다. 경로가 누락되면 하드 오류입니다.
2. **빌드 시 포함 경로** — `SHIRABE_BROWSER_PATH`. `build.rs`에 의해 생성되며,
   `auto-fetch` 기능이 컴파일 중 고정된 Chrome for Testing 빌드를
   공유 캐시로 다운로드할 때 설정됩니다.
3. **시스템 바이너리** — `$PATH` 상의 바이너리 및 잘 알려진 설치 위치
   (`/usr/bin/google-chrome`, `/Applications/Google Chrome.app/...`,
   `C:\Program Files\Google\Chrome\Application\chrome.exe`, …).
4. **런타임 페치** (`runtime-fetch` 기능) — 최초 사용 시 고정된
   Chrome for Testing 빌드를 캐시로 다운로드합니다.

## 다운로드 설정

페치 단계는 빌드 시(`build.rs`)와 런타임 시 모두 다음 환경 변수를
참조합니다:

| Env | Purpose |
|-----|---------|
| `SHIRABE_CHROME_VERSION` | 고정된 Chrome for Testing 버전을 재정의합니다. |
| `SHIRABE_CHROME_MIRROR` | 기본 Google 호스트 대신 미러(예: GFW 친화적 미러)에서 다운로드합니다. |
| `SHIRABE_CHROME_SHA256` | 선택적 16진수 체크섬. 다운로드가 이에 대해 검증됩니다. |
| `SHIRABE_DOWNLOAD_PROXY` | `http://`, `https://` 또는 `socks5://` 프록시를 통해 다운로드를 라우팅합니다. |
| `SHIRABE_DOWNLOAD_TIMEOUT_SECS` | 요청별 타임아웃 (기본값 600). |
| `SHIRABE_SKIP_BROWSER_FETCH` | 빌드 시와 런타임 시 다운로드를 모두 건너뜁니다. |

> `build.rs`도 이를 읽으므로, 하위 크레이트는 CI에서 단일 `env:` 블록으로
> 전체 툴체인을 고정할 수 있습니다.
