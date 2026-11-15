# الخلفيات ودقة الحل

يقوم shirabe بتشغيل أي متصفح يتحدث بروتوكول Chrome DevTools — Google
Chrome و Chromium و Microsoft Edge — من خلال محرك CDP واحد. اختر واحدًا باستخدام
`SHIRABE_BACKEND`:

| Value | Backend |
|-------|---------|
| `chrome` (default in `auto`) | Google Chrome |
| `chromium` | Chromium |
| `edge` | Microsoft Edge |
| `auto` (default) | Try Chrome, then Chromium, then Edge |

## ترتيب الدقة

أيًا كانت الخلفية المختارة، يحل shirabe الملف التنفيذي بهذا الترتيب
(محاكيًا نموذج التبعية الخاص بـ [ort](https://crates.io/crates/ort)):

1. **تجاوز خاص بالخلفية** — `CHROME_PATH` / `CHROMIUM_PATH` / `EDGE_PATH`.
   إذا تم تعيينه، يكون المسار موثوقًا؛ والمسار المفقود يُعتبر خطأً قاطعًا.
2. **مسار مضمن في وقت البناء** — `SHIRABE_BROWSER_PATH`، ينشأ بواسطة `build.rs`
   عندما تقوم ميزة `auto-fetch` بتنزيل بنية Chrome for Testing المثبتة
   إلى الذاكرة المؤقتة المشتركة أثناء التجميع.
3. **ثنائي النظام** على `$PATH` بالإضافة إلى مجموعة من مواقع التثبيت المعروفة
   (`/usr/bin/google-chrome`، `/Applications/Google Chrome.app/...`،
   `C:\Program Files\Google\Chrome\Application\chrome.exe`، …).
4. **جلب وقت التشغيل** (ميزة `runtime-fetch`) — تنزيل بنية Chrome for Testing
   المثبتة عند أول استخدام إلى الذاكرة المؤقتة.

## مقابض التنزيل

تراعي خطوة الجلب متغيرات البيئة التالية، سواء في وقت البناء
(`build.rs`) أو في وقت التشغيل:

| Env | Purpose |
|-----|---------|
| `SHIRABE_CHROME_VERSION` | تجاوز إصدار Chrome for Testing المثبت. |
| `SHIRABE_CHROME_MIRROR` | التنزيل من مرآة (مثلاً مرآة صديقة لجدار الحماية العظيم) بدلاً من مضيف Google الافتراضي. |
| `SHIRABE_CHROME_SHA256` | في وقت البناء يكون المجموع التدقيقي إلزاميًا افتراضيًا: يثبّت build.rs بصمة sha256 لكل أرشيف ويُجهض عند عدم التطابق أو غياب التثبيت. وفي وقت التشغيل يقود الجدول المثبّت نفسه علامة الثقة في الذاكرة المؤقتة؛ أما النسخة/التركيبة غير المثبّتة فتبقى بلا تحقّق، وفشل إعادة التثبيت لشجرة غير موثوقة يعود إليها بتحذير بدل الخطأ (وجّه CHROME_PATH إلى بناء موثوق لتجنّب ذلك). هذا المتغير يتجاوز البصمة المثبّتة فقط. |
| `SHIRABE_DOWNLOAD_PROXY` | توجيه التنزيل عبر وكيل `http://` أو `https://` أو `socks5://`. |
| `SHIRABE_DOWNLOAD_TIMEOUT_SECS` | مهلة لكل طلب (الافتراضي 600). |
| `SHIRABE_SKIP_BROWSER_FETCH` | تخطي عمليات التنزيل في وقت البناء ووقت التشغيل. |

> نظرًا لأن `build.rs` يقرأ هذه أيضًا، يمكن لصندوق تابع تثبيت سلسلة
> الأدوات بأكملها في CI باستخدام كتلة `env:` واحدة.
