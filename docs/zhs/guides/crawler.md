# 爬虫编排（shirabe-crawler）

`shirabe-crawler` 是建在 shirabe 之上的**爬虫编排层**:URL 队列、并发 worker、按主机限速、
基于 schema 的结构化提取、可插拔存储。它通过一个 `PageDriver` trait 驱动浏览器,**默认后端就是
shirabe 自己的无头浏览器**。

## 为什么是独立 crate

shirabe 是 CDP 引擎 + 调试 API —— 浏览器**底座**。爬虫是建在底座之上的**编排**:调度、限速、
提取、存储。两者分开的意义:

- **shirabe 保持纯粹的底座。** 它的公开 API、依赖、职责不会因此膨胀。
- **爬虫与底座解耦。** 爬虫只认 `PageDriver` trait。今天用 shirabe 真浏览器跑,明天换成池化/远程
  driver,编排逻辑一行不用改。

这复刻了 shirabe 自己"后端可换"的设计(`ort` 与其 provider):**爬虫之于 `PageDriver`,如同
shirabe 之于它的浏览器后端。**

## 架构

```text
  ┌──────────── 爬虫（编排层）────────────┐
  │  frontier · workers · politeness · extract · sink │
  └───────────────────────┬───────────────────────────┘
                          │ PageDriver（唯一接缝）
  ┌───────────────────────▼───────────────────────────┐
  │  shirabe 调试 API   ←┄┄ 可替换 ┄┄→   mock / 自定义 │
  └─────────────────────────────────────────────────────┘
```

## 快速开始

先起一个 shirabe 调试服务（`shirabe debug --port 3001`），再驱动它爬：

```rust
use std::sync::Arc;
use std::time::Duration;
use shirabe_crawler::{
    Crawler, MemorySink, PolitenessConfig, ShirabeDriver, ShirabeDriverConfig,
    WorkerJob, ExtractionSchema, FieldSource, FieldSpec,
};

# async fn run() -> anyhow::Result<()> {
let driver = Arc::new(ShirabeDriver::new(ShirabeDriverConfig {
    endpoint: "http://localhost:3001".into(),
    timeout: Duration::from_secs(30),
})?);

let sink = Arc::new(MemorySink::new());

let schema = ExtractionSchema {
    container: Some("article.post".into()),
    fields: vec![
        FieldSpec { name: "title".into(), selector: "h2".into(), source: FieldSource::Text },
        FieldSpec { name: "url".into(),   selector: "a".into(),  source: FieldSource::Attr { name: "href".into() } },
    ],
};

let crawler = Crawler::builder()
    .driver(driver)
    .sink(sink.clone())
    .politeness(PolitenessConfig { per_host_delay: Duration::from_secs(1), ..Default::default() })
    .concurrency(2)
    .job(WorkerJob { extract: Some(schema), follow_links: true })
    .build()?;

crawler.seed(["https://example.com/".into()]).await;
let visited = crawler.run().await?;
println!("爬取了 {} 个页面", visited);
# Ok(())
# }
```

## 各模块职责

| 模块 | 职责 |
|---|---|
| `driver` | `PageDriver` 接缝：`fetch` 一个 URL，在页面里 `evaluate` JS。 |
| `drivers::shirabe` | 默认后端 —— 驱动 shirabe 的 HTTP 调试 API。 |
| `drivers::mock` | 内存后端，用于测试 / 离线开发。 |
| `frontier` | 带优先级的 URL 队列，含去重、深度上限、scheme 白名单。 |
| `politeness` | 按主机限速 + 并发上限、指数退避、重试建议。 |
| `extract` | 声明式 schema（`container` + 字段选择器），编译成 JS，经 driver 执行。 |
| `link_discovery` | 把页面的 `<a href>` 归一化为绝对地址喂回 frontier。 |
| `sink` | `RecordSink` trait + 内存 / NDJSON 文件 sink。 |
| `worker` | fetch→提取→发现链接→落 sink 的循环，N 个并发。 |

## 写自己的后端

实现 `PageDriver` 的两个方法即可：

```rust
use async_trait::async_trait;
use shirabe_crawler::{PageDriver, FetchedPage, CrawlError};

pub struct MyDriver;

#[async_trait]
impl PageDriver for MyDriver {
    async fn fetch(&self, url: &str) -> Result<FetchedPage, CrawlError> {
        // 用你自己的浏览器/HTTP 栈渲染页面
#       unimplemented!()
    }
    async fn evaluate(
        &self, expression: &str, _page: &FetchedPage,
    ) -> Result<serde_json::Value, CrawlError> {
        // 执行 JS（无 JS 能力的后端返回 Err，爬虫会优雅降级）
#       unimplemented!()
    }
}
```

爬虫核心（frontier / worker / politeness / extract）不感知你用了什么浏览器。

## 解耦边界（给 reviewer）

- shirabe 本体（`src/`）源码零改动；唯一改动是根 `Cargo.toml` 增加 workspace 段。
- 本 crate 只依赖 shirabe 的**公开** API 与 HTTP 调试 API，不触及任何 `pub(crate)` 符号。
- 删掉 `crates/shirabe-crawler/` 即可完全回退，shirabe 无残留。

## 许可证

SySL-1.0（Synthetic Source License）。本 crate 有部分代码在 AI 模型辅助下生成，披露按协议条款保留。
