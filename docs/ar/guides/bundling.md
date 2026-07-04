# تجميع المكتبات الأصلية

عند شحن منتج مبني على shirabe، عادةً ما يحتاج صنفان من الملفات الأصلية إلى مرافقة الملف الثنائي:

1. **اعتماديات وقت تشغيل خلفية المتصفح.** نسخة Chrome for Testing التي يتم جلبها ترتبط بمكتبات النظام (`libnss3.so`، `libdbus-1.so`، …) والتي قد لا تتوفر في حاوية نظيفة.
2. **اعتمادياتك الأصلية** — ملفات `.so` / `.dylib` / `.dll` التي يرتبط بها crate الخاص بك.

توفر shirabe للمُجمِّعين وحدة واحدة — `shirabe::bundle` — للتعامل مع كليهما.

## تحديد ما يجب شحنه

أدرج الملفات حرفيًا باستخدام `SHIRABE_BUNDLE_LIBS` (قائمة مفصولة بفاصل المسار: `:` على Unix، `;` على Windows):

```bash
SHIRABE_BUNDLE_LIBS="/opt/myapp/libfoo.so:/opt/myapp/libbar.so"
```

أو اكتب ملف `bundle.toml` وأشر إليه باستخدام `SHIRABE_BUNDLE_MANIFEST`:

```toml
[[lib]]
path = "third_party/libfoo.so"
optional = true
target_os = "linux"

[[lib]]
path = "third_party/foo.dll"
```

يتم دمج كلا المصدرين بواسطة `BundleSpec::from_env()`.

## اكتشاف ما يجب شحنه

`collect_runtime_deps(exe)` يفحص ملفًا ثنائيًا بحثًا عن اعتماديات المكتبات المشتركة — `ldd` على Linux، `otool -L` على macOS، وفحص استيراد PE بأفضل جهد على Windows — ويعيد كل اعتمادية مسجلة مع المكان الذي وجدها فيه المحلل.

## تجميع كل شيء معًا

`BundleReport::build(&backend_exe)` يدمج الحزمة المُعلنة مع الاعتماديات المكتشفة من الملف التنفيذي للخلفية المُحلل، ويقوم `render_bundle_report(&report)` بتحويلها إلى إرشادات قابلة للقراءة البشرية يمكن لسكريبت النشر طباعتها أو كتابتها في ملف بيان:

```rust
use shirabe::{BundleReport, render_bundle_report};

let report = BundleReport::build(&backend_exe);
print!("{}", render_bundle_report(&report));
```

يمكن لسكريبت النشر بعد ذلك نسخ كل مسار `resolved` (وكل مكتبة مُعلنة وغير اختيارية) إلى دليل التوزيع، منتجًا منتجًا قائمًا بذاته يعمل على جهاز بدون Chrome أو مكتبات النظام المثبتة عليه.
