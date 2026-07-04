# 빠른 시작

## CLI

```bash
# Zero-config: auto-discovers Chrome/Chromium/Edge, or fetches Chrome for Testing.
shirabe debug --port 3001

# Pin a backend, route the browser through a proxy.
SHIRABE_BACKEND=chromium shirabe debug --port 3001 --proxy http://localhost:7890
```

그런 다음 HTTP를 통해 실행 중인 서버를 구동합니다:

```bash
curl -X POST http://localhost:3001/navigate \
  -H "Content-Type: application/json" -d '{"url":"https://example.com"}'

curl -X POST http://localhost:3001/screenshot -d '{}'
```

## 라이브러리

```rust
use shirabe::{start_debug_server, DebugServerConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = DebugServerConfig {
        base_url: "about:blank".to_string(),
        dev_port: 0,
        dist_dir: String::new(),
        package_name: String::new(),
        proxy: None,
    };
    start_debug_server(cfg, 3001).await
}
```

실행 파일을 찾는 방법은 [백엔드 및 해결](./backends.md)을, 독립형 제품을 배포하는 방법은 [네이티브 라이브러리 번들링](./bundling.md)을 참조하세요.
