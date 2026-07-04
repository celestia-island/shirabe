# Быстрый старт

## CLI

```bash
# Zero-config: auto-discovers Chrome/Chromium/Edge, or fetches Chrome for Testing.
shirabe debug --port 3001

# Pin a backend, route the browser through a proxy.
SHIRABE_BACKEND=chromium shirabe debug --port 3001 --proxy http://localhost:7890
```

Затем управляйте запущенным сервером через HTTP:

```bash
curl -X POST http://localhost:3001/navigate \
  -H "Content-Type: application/json" -d '{"url":"https://example.com"}'

curl -X POST http://localhost:3001/screenshot -d '{}'
```

## Библиотека

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

См. [Бэкенды и разрешение](./backends.md) о том, как находится исполняемый файл, и [Упаковка нативных библиотек](./bundling.md) о поставке автономного продукта.
