# المحركات الخارجية — Firefox & Servo

عائلة Chromium (Chrome / Chromium / Edge) تُدار داخل العملية عبر محرك CDP الخاص
بـ shirabe. أما **Firefox** و **Servo** فيسلكان مسارًا مختلفًا: نواتهما ضخمة،
لذا نسمح لموفري المتصفح (أو أي شخص يبني هذه النوى) بتجميع محوّل صغير مقابل
C ABI ثابت وشحنه كمكتبة ديناميكية — وهو نفس النموذج الذي تستخدمه
[ort](https://crates.io/crates/ort) مع ONNX Runtime. shirabe هو "غلاف ربط C رفيع":
يفتح المكتبة الديناميكية للمزوّد ويوجّه النداءات عبر trait عام هو
[`Engine`](https://shirabe.docs.celestia.world).

```
your app ── shirabe (CDP engine) ── Chrome / Chromium / Edge   (in-process)
        └─ shirabe (FFI wrapper) ── libshirabe_engine_firefox ── Firefox core
                                 └ libshirabe_engine_servo   ── Servo core
```

## التفعيل

```toml
shirabe = { version = "0.1", features = ["foreign-engine"] }
```

ثم اختر واجهة خلفية خارجية:

```bash
SHIRABE_BACKEND=firefox shirabe debug --port 3001
SHIRABE_BACKEND=servo   shirabe debug --port 3001
```

## C ABI الذي تصدّره مكتبة المزوّد

```c
typedef struct shirabe_engine shirabe_engine;

shirabe_engine *shirabe_engine_new(const char *options_json);   /* JSON opts   */
void  shirabe_engine_destroy(shirabe_engine *eng);
int   shirabe_engine_navigate(shirabe_engine *eng, const char *url);
char *shirabe_engine_evaluate(shirabe_engine *eng, const char *js); /* JSON out */
void  shirabe_engine_free_string(shirabe_engine *eng, char *s);
int   shirabe_engine_screenshot(shirabe_engine *eng,
                                unsigned char **out, size_t *out_len);   /* PNG */
void  shirabe_engine_free_pixels(shirabe_engine *eng, unsigned char *buf, size_t len);
const char *shirabe_engine_id(void);                              /* "firefox" … */
```

بضع مئات من أسطر كود المحوّل مقابل هذا الـ ABI تكفي لتشغيل نواة متصفح كاملة؛
كل ما يعرضه shirabe عبر HTTP مبني على هذه العمليات الخمس.

## من أين تأتي المكتبة

`CdylibEngine::open` يبحث عن `libshirabe_engine_<id>.{so,dylib,dll}` في:

1. `SHIRABE_ENGINE_PATH` — تجاوز صريح.
2. بجوار الملف التنفيذي الحالي.
3. `<cache>/shirabe/engines/<id>/` — حيث تضع خطوة جلب الإصدار النسخ المُنزّلة
   (ينشر سير عمل الإصدار مكتبات مبنية مسبقًا على GitHub Releases تحت وسومها
   الخاصة).

حتى ينشر مزوّد مكتبة، فإن اختيار Firefox/Servo يُرجع خطأً واضحًا يشير إلى عقد FFI
— لا يحاول shirabe أبدًا تشغيل `firefox` كما لو كان يتحدث CDP.
