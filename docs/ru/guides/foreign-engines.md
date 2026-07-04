# Внешние движки — Firefox & Servo

Семейство Chromium (Chrome / Chromium / Edge) управляется внутри процесса через
собственный CDP-движок shirabe. **Firefox** и **Servo** идут другим путём: их
ядра огромны, поэтому мы позволяем производителям браузеров (или всем, кто
собирает эти ядра) скомпилировать небольшой адаптер под фиксированный C ABI и
поставлять его в виде динамической библиотеки — та же модель, которую
[ort](https://crates.io/crates/ort) использует для ONNX Runtime. shirabe — это
«тонкая C-обёртка»: она загружает библиотеку производителя через dlopen и
маршрутизирует вызовы через обобщённый типаж
[`Engine`](https://shirabe.docs.celestia.world).

```
your app ── shirabe (CDP engine) ── Chrome / Chromium / Edge   (in-process)
        └─ shirabe (FFI wrapper) ── libshirabe_engine_firefox ── Firefox core
                                 └ libshirabe_engine_servo   ── Servo core
```

## Включение

```toml
shirabe = { version = "0.1", features = ["foreign-engine"] }
```

Затем выберите внешний бэкенд:

```bash
SHIRABE_BACKEND=firefox shirabe debug --port 3001
SHIRABE_BACKEND=servo   shirabe debug --port 3001
```

## C ABI, экспортируемый библиотекой производителя

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

Нескольких сотен строк кода адаптера под этот ABI достаточно для управления целым
ядром браузера; всё, что shirabe предоставляет через HTTP, построено на этих пяти
операциях.

## Откуда берётся библиотека

`CdylibEngine::open` ищет `libshirabe_engine_<id>.{so,dylib,dll}` в:

1. `SHIRABE_ENGINE_PATH` — явное переопределение.
2. рядом с текущим исполняемым файлом.
3. `<cache>/shirabe/engines/<id>/` — куда шаг получения релиза помещает
   загруженные копии (рабочий процесс публикации загружает предсобранные
   библиотеки в GitHub Releases под их собственными тегами).

Пока производитель не опубликует библиотеку, выбор Firefox/Servo возвращает
понятную ошибку, указывающую на контракт FFI — shirabe никогда не пытается
запустить `firefox` так, будто он говорит CDP.
