# 快速入门

## 命令行工具

```bash
# Zero-config: auto-discovers Chrome/Chromium/Edge, or fetches Chrome for Testing.
shirabe debug --port 3001

# Pin a backend, route the browser through a proxy.
SHIRABE_BACKEND=chromium shirabe debug --port 3001 --proxy http://localhost:7890
```

然后通过 HTTP 驱动正在运行的服务器：

```bash
curl -X POST http://localhost:3001/navigate \
  -H "Content-Type: application/json" -d '{"url":"https://example.com"}'

curl -X POST http://localhost:3001/screenshot -d '{}'
```

## 库

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

有关如何查找可执行文件，请参阅[后端与解析](./backends.md)；有关打包独立产品，请参阅[捆绑原生库](./bundling.md)。
