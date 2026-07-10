<p align="center"><img src="https://raw.githubusercontent.com/celestia-island/shirabe/master/docs/logo.webp" alt="Shirabe" width="240" /></p>

<h1 align="center">Shirabe</h1>

<p align="center"><strong>Автоматизация безголового браузера</strong></p>

<div align="center">

[![License: SySL-1.0](https://img.shields.io/badge/License-SySL--1.0-blue.svg)](https://sysl.celestia.world)
[![GitHub](https://img.shields.io/badge/github-celestia--island%2Fshirabe-blue.svg)](https://github.com/celestia-island/shirabe)
[![Checks](https://img.shields.io/github/actions/workflow/status/celestia-island/shirabe/checks.yml)](https://github.com/celestia-island/shirabe/actions/workflows/checks.yml)
[![Docs](https://img.shields.io/badge/docs-shirabe.docs.celestia.world-blue)](https://shirabe.docs.celestia.world)
[![docs.rs](https://docs.rs/shirabe/badge.svg)](https://docs.rs/shirabe)

</div>

<div align="center">

[English](../en/README.md) ·
[简体中文](../zhs/README.md) ·
[繁體中文](../zht/README.md) ·
[日本語](../ja/README.md) ·
[한국어](../ko/README.md) ·
[Français](../fr/README.md) ·
[Español](../es/README.md) ·
**Русский** ·
[العربية](../ar/README.md)

</div>

## Введение

shirabe — это лёгкая Rust-нативная библиотека автоматизации браузера и отладочный
сервер. Она управляет любым браузером, говорящим на протоколе Chrome DevTools —
Google Chrome, Chromium, Microsoft Edge — через один собственноручно написанный
движок CDP и предоставляет всё это через небольшой HTTP-API. Это браузерная
основа, выделенная из упаковщика tairitsu и укреплённая для самостоятельной
работы.

Основная идея та же, что у [ort](https://crates.io/crates/ort) для ONNX Runtime:
**вам никогда не придётся устанавливать браузер вручную.** Зафиксированная сборка
Chrome for Testing загружается в общий кэш во время сборки (или при первом
использовании), прозрачно находится и управляется через CDP. Укажите другой
бэкенд, поставляйте нативные библиотеки с вашим продуктом, направляйте загрузку
через зеркало или прокси — всё через переменные окружения.

## Быстрый старт

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

### Библиотека

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

## Бэкенды и резолвер без настройки

Выберите бэкенд с помощью `SHIRABE_BACKEND=chrome|chromium|edge|firefox|servo|auto`
(по умолчанию `auto`). **Семейство Chromium** (Chrome / Chromium / Edge)
управляется внутри процесса через наш собственный движок CDP; **Firefox** и
**Servo** идут другим путём — их ядра собираются производителями браузеров и
поставляются как динамические библиотеки, которыми shirabe управляет через
тонкий FFI-контракт на C-биндингах (фича `foreign-engine`, см.
[Внешние движки](../en/guides/foreign-engines.md)). Какой бы ни был выбран,
shirabe разрешает его в следующем порядке:

1. **Переопределение для конкретного бэкенда** — `CHROME_PATH` / `CHROMIUM_PATH` / `EDGE_PATH`.
2. **Путь, запечённый при сборке** — `SHIRABE_BROWSER_PATH`, записываемый `build.rs`,
   когда фича `auto-fetch` загружает Chrome for Testing во время сборки.
3. **Системный бинарник** в `$PATH` и известных местах установки.
4. **Загрузка во время выполнения** (фича `runtime-fetch`) — загрузка
   зафиксированной сборки в общий кэш.

Параметры загрузки (как при сборке, так и при выполнении):

| Переменная окружения | Назначение |
|----------------------|------------|
| `SHIRABE_CHROME_VERSION` | Переопределить зафиксированную версию Chrome for Testing. |
| `SHIRABE_CHROME_MIRROR` | Загружать с зеркала вместо `storage.googleapis.com`. |
| `SHIRABE_CHROME_SHA256` | Опциональная шестнадцатеричная контрольная сумма для проверки загрузки. |
| `SHIRABE_DOWNLOAD_PROXY` | Направить загрузку через `http://` / `https://` / `socks5://` прокси. |
| `SHIRABE_DOWNLOAD_TIMEOUT_SECS` | Таймаут на запрос (по умолчанию 600). |
| `SHIRABE_SKIP_BROWSER_FETCH` | Пропустить загрузку как при сборке, так и при выполнении. |
| `SHIRABE_BACKEND` | Какой бэкенд семейства Chromium использовать. |

## Поставка нативных библиотек с вашим продуктом

Загруженная сборка Chrome (и ваш собственный крейт) зависят от нативных
библиотек, которые могут отсутствовать в чистом контейнере. shirabe даёт
упаковщикам два инструмента:

- **Декларативный набор** — перечислите файлы `.so` / `.dylib` / `.dll` для
  поставки через `SHIRABE_BUNDLE_LIBS` (список, разделённый разделителем пути)
  или `SHIRABE_BUNDLE_MANIFEST` (файл `bundle.toml` с таблицами `[[lib]]`).
  Пример манифеста:

  ```toml
  [[lib]]
  path = "third_party/libfoo.so"
  optional = true
  target_os = "linux"

  [[lib]]
  path = "third_party/foo.dll"
  ```

- **Сканирование зависимостей** — `shirabe::collect_runtime_deps(exe)` перечисляет
  разделяемые библиотеки, с которыми слинкован бинарник (`ldd` / `otool -L` /
  сканирование импорта PE), а `shirabe::render_bundle_report(&BundleReport::build(&exe))`
  выводит всё, что релизный скрипт должен скопировать рядом с бинарником.

```rust
use shirabe::{BundleReport, render_bundle_report};
let report = BundleReport::build(&backend_exe);
print!("{}", render_bundle_report(&report));
```

## HTTP API

| Метод  | Путь               | Описание                          |
|--------|--------------------|-----------------------------------|
| `GET`  | `/health`          | Состояние сервера                 |
| `GET`  | `/info`            | Статус браузера + выбранный бэкенд |
| `POST` | `/navigate`        | Перейти по URL                    |
| `POST` | `/click`           | Кликнуть по элементу              |
| `POST` | `/type`            | Ввести текст                      |
| `POST` | `/evaluate`        | Выполнить JavaScript              |
| `POST` | `/screenshot`      | Сделать скриншот                  |
| `POST` | `/wait-for-selector` | Ожидать элемент                 |
| `GET`  | `/dom`             | Запросить DOM                     |
| `GET`  | `/a11y`             | Дерево доступности               |
| `POST` | `/batch`           | Пакетные операции                 |

…плюс конечные точки для захвата консоли, сети и websocket для полного контроля.

## MCP-сервер

Соберите shirabe с feature `mcp` и запустите stdio-сервер — он размещает API отладки headless-браузера в процессе и предоставляет его операции AI-ассистентам программиста по протоколу Model Context Protocol:

```bash
shirabe mcp
```

Сервер предоставляет двенадцать инструментов — каждый проксирует через loopback к внутри-процессному CDP-движку.

```json
{
  "mcpServers": {
    "shirabe": { "command": "shirabe", "args": ["mcp"] }
  }
}
```

Установите `SHIRABE_URL` и `SHIRABE_DOWNLOAD_PROXY` при необходимости.

## Разработка

```bash
SHIRABE_SKIP_BROWSER_FETCH=1 cargo clippy --all-targets --all-features -- -D warnings
SHIRABE_SKIP_BROWSER_FETCH=1 cargo test --all-features
```

## Лицензия

SySL-1.0 (Synthetic Source License). См. [LICENSE](https://sysl.celestia.world).
