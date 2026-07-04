# Бэкенды и разрешение

shirabe управляет любым браузером, поддерживающим протокол Chrome DevTools —
Google Chrome, Chromium, Microsoft Edge — через единый движок CDP. Выберите
один с помощью `SHIRABE_BACKEND`:

| Value | Backend |
|-------|---------|
| `chrome` (default in `auto`) | Google Chrome |
| `chromium` | Chromium |
| `edge` | Microsoft Edge |
| `auto` (default) | Try Chrome, then Chromium, then Edge |

## Порядок разрешения

Какой бы бэкенд ни был выбран, shirabe разрешает исполняемый файл в этом порядке
(отражая модель зависимостей [ort](https://crates.io/crates/ort)):

1. **Переопределение для конкретного бэкенда** — `CHROME_PATH` / `CHROMIUM_PATH` /
   `EDGE_PATH`. Если задано, путь имеет приоритет; отсутствующий путь —
   критическая ошибка.
2. **Встроенный при сборке путь** — `SHIRABE_BROWSER_PATH`, генерируется
   `build.rs`, когда функция `auto-fetch` загружает закреплённую сборку Chrome
   for Testing в общий кэш во время компиляции.
3. **Системный бинарный файл** в `$PATH`, а также несколько известных мест
   установки (`/usr/bin/google-chrome`,
   `/Applications/Google Chrome.app/...`,
   `C:\Program Files\Google\Chrome\Application\chrome.exe`, …).
4. **Загрузка во время выполнения** (функция `runtime-fetch`) — загрузка
   закреплённой сборки Chrome for Testing в кэш при первом использовании.

## Параметры загрузки

Шаг загрузки учитывает следующие переменные окружения как во время сборки
(`build.rs`), так и во время выполнения:

| Env | Purpose |
|-----|---------|
| `SHIRABE_CHROME_VERSION` | Переопределяет закреплённую версию Chrome for Testing. |
| `SHIRABE_CHROME_MIRROR` | Загрузка с зеркала (например, совместимого с GFW) вместо хоста Google по умолчанию. |
| `SHIRABE_CHROME_SHA256` | Опциональная шестнадцатеричная контрольная сумма; загрузка сверяется с ней. |
| `SHIRABE_DOWNLOAD_PROXY` | Маршрутизация загрузки через прокси `http://`, `https://` или `socks5://`. |
| `SHIRABE_DOWNLOAD_TIMEOUT_SECS` | Тайм-аут на запрос (по умолчанию 600). |
| `SHIRABE_SKIP_BROWSER_FETCH` | Пропустить загрузки как во время сборки, так и во время выполнения. |

> Поскольку `build.rs` также читает их, downstream-крейт может зафиксировать
> всю цепочку инструментов в CI с помощью одного блока `env:`.
