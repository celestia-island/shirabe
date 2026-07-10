<p align="center"><img src="https://raw.githubusercontent.com/celestia-island/shirabe/master/docs/logo.webp" alt="Shirabe" width="240" /></p>

<h1 align="center">Shirabe</h1>

<p align="center"><strong>أتمتة المتصفح بدون رأس</strong></p>

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
[Русский](../ru/README.md) ·
**العربية**

</div>

## مقدمة

shirabe مكتبة أتمتة متصفح خفيفة مكتوبة بلغة Rust وخادم تنقيح. تقود أي متصفح يتحدث
بروتوكول Chrome DevTools — Google Chrome وChromium وMicrosoft Edge — عبر محرّك
CDP واحد مكتوب يدويًا، وتعرض كل ذلك عبر واجهة HTTP API صغيرة. إنها البنية الأساسية
للمتصفح المستخرجة من حزمة tairitsu، والمُعزَّزة لتعمل بشكل مستقل.

الفكرة الموجهة هي نفسها فكرة [ort](https://crates.io/crates/ort) لـ ONNX Runtime:
**لا ينبغي أن تضطر أبدًا لتثبيت متصفح يدويًا.** يتم جلب إصدار مُثبَّت من Chrome for
Testing إلى ذاكرة تخزين مؤقت مشتركة في وقت البناء (أو عند أول استخدام)، ويتم تحديد
موقعه بشفافية، ثم قيادته عبر CDP. ثبّت خلفية مختلفة، وشحن المكتبات الأصلية مع منتجك،
ووجّه التحميل عبر مرآة أو وكيل — كل ذلك من متغيرات البيئة.

## البدء السريع

### واجهة سطر الأوامر

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

### مكتبة

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

## الخلفيات والتحليل التلقائي بدون إعداد

اختر خلفية باستخدام `SHIRABE_BACKEND=chrome|chromium|edge|firefox|servo|auto`
(الافتراضي `auto`). تُقاد **عائلة Chromium** (Chrome / Chromium / Edge) داخل
العملية عبر محرّك CDP الخاص بنا؛ أما **Firefox** و**Servo** فيسلكان مسارًا مختلفًا —
حيث تُبنى نواتهما من قبل مُطوّري المتصفحات وتُشحن كمكتبات ديناميكية، وتقودها shirabe
عبر عقد FFI رفيع بربط C (ميزة `foreign-engine`، راجع
[المحركات الخارجية](../en/guides/foreign-engines.md)). أيًا كان الخيار المختار، تحلّه
shirabe بهذا الترتيب:

1. **تجاوز خاص بالخلفية** — `CHROME_PATH` / `CHROMIUM_PATH` / `EDGE_PATH`.
2. **مسار مُدمج في وقت البناء** — `SHIRABE_BROWSER_PATH`، يُصدره `build.rs`
   عندما تقوم ميزة `auto-fetch` بتحميل Chrome for Testing أثناء البناء.
3. **ثنائي النظام** على `$PATH` ومواقع التثبيت المعروفة.
4. **جلب وقت التشغيل** (ميزة `runtime-fetch`) — تحميل الإصدار المُثبَّت إلى
   الذاكرة المخبأة المشتركة.

مفاتيح التحميل (في وقت البناء ووقت التشغيل على حد سواء):

| المتغير | الغرض |
|---------|-------|
| `SHIRABE_CHROME_VERSION` | تجاوز إصدار Chrome for Testing المُثبَّت. |
| `SHIRABE_CHROME_MIRROR` | التحميل من مرآة بدلاً من `storage.googleapis.com`. |
| `SHIRABE_CHROME_SHA256` | بصمة SHA256 سداسية عشرية اختيارية للتحقق من التحميل. |
| `SHIRABE_DOWNLOAD_PROXY` | توجيه التحميل عبر وكيل `http://` / `https://` / `socks5://`. |
| `SHIRABE_DOWNLOAD_TIMEOUT_SECS` | مهلة لكل طلب (الافتراضي 600). |
| `SHIRABE_SKIP_BROWSER_FETCH` | تخطّي التحميل في وقت البناء ووقت التشغيل معًا. |
| `SHIRABE_BACKEND` | أي خلفية من عائلة Chromium تُقاد. |

## شحن المكتبات الأصلية مع منتجك

يعتمد بناء Chrome المُجلوب (والصندوق الخاص بك) على مكتبات أصلية قد يفتقر إليها
حاوي نظيف. تمنح shirabe المُعبّئين أداتين:

- **حزمة تعريفية** — اسرد ملفات `.so` / `.dylib` / `.dll` للشحن عبر
  `SHIRABE_BUNDLE_LIBS` (قائمة مفصولة بفاصل المسار) أو `SHIRABE_BUNDLE_MANIFEST`
  (ملف `bundle.toml` يحتوي جداول `[[lib]]`). مثال على البيان:

  ```toml
  [[lib]]
  path = "third_party/libfoo.so"
  optional = true
  target_os = "linux"

  [[lib]]
  path = "third_party/foo.dll"
  ```

- **فحص التبعيات** — `shirabe::collect_runtime_deps(exe)` تُعدّد المكتبات المشتركة
  التي يرتبط بها ثنائي (`ldd` / `otool -L` / فحص استيراد PE)، و
  `shirabe::render_bundle_report(&BundleReport::build(&exe))` تطبع كل ما يجب على
  سكريبت الإصدار نسخه بجانب الثنائي.

```rust
use shirabe::{BundleReport, render_bundle_report};
let report = BundleReport::build(&backend_exe);
print!("{}", render_bundle_report(&report));
```

## واجهة HTTP API

| الطريقة | المسار | الوصف |
|---------|--------|-------|
| `GET`  | `/health` | صحة الخادم |
| `GET`  | `/info` | حالة المتصفح + الخلفية المختارة |
| `POST` | `/navigate` | الانتقال إلى عنوان URL |
| `POST` | `/click` | النقر على عنصر |
| `POST` | `/type` | كتابة نص |
| `POST` | `/evaluate` | تنفيذ JavaScript |
| `POST` | `/screenshot` | التقاط لقطة شاشة |
| `POST` | `/wait-for-selector` | انتظار عنصر |
| `GET`  | `/dom` | استعلام DOM |
| `GET`  | `/a11y` | شجرة إمكانية الوصول |
| `POST` | `/batch` | عمليات دفعية |

…بالإضافة إلى نقاط نهاية console وnetwork وwebsocket للتحكم الكامل.

## خادم MCP

ابنِ shirabe بميزة `mcp` وشغّل خادم stdio — فهو يستضيف واجهة برمجة تصحيح المتصفح مقطوع الرأس داخل العملية ويعرض عملياته لمساعدي الترميز بالذكاء الاصطناعي عبر بروتوكول سياق النموذج:

```bash
shirabe mcp
```

يُعلن الخادم عن اثني عشر أداة — كل منها يُمرر عبر الاسترجاع إلى محرك CDP داخل العملية.

```json
{
  "mcpServers": {
    "shirabe": { "command": "shirabe", "args": ["mcp"] }
  }
}
```

عيّن `SHIRABE_URL` و`SHIRABE_DOWNLOAD_PROXY` حسب الحاجة.

## التطوير

```bash
SHIRABE_SKIP_BROWSER_FETCH=1 cargo clippy --all-targets --all-features -- -D warnings
SHIRABE_SKIP_BROWSER_FETCH=1 cargo test --all-features
```

## الترخيص

SySL-1.0 (رخصة المصدر الاصطناعي). راجع [LICENSE](https://sysl.celestia.world).
