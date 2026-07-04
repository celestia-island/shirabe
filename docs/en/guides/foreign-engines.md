# Foreign engines — Firefox & Servo

The Chromium family (Chrome / Chromium / Edge) is driven in-process through
shirabe's own CDP engine. **Firefox** and **Servo** take a different path: their
cores are huge, so we let the browser vendors (or anyone building those cores)
compile a tiny adapter against a fixed C ABI and ship it as a dynamic library —
the same model [ort](https://crates.io/crates/ort) uses for ONNX Runtime.
shirabe is the "thin C-binding wrapper": it dlopens the vendor lib and routes
calls through a generic [`Engine`](https://shirabe.docs.celestia.world) trait.

```
your app ── shirabe (CDP engine) ── Chrome / Chromium / Edge   (in-process)
        └─ shirabe (FFI wrapper) ── libshirabe_engine_firefox ── Firefox core
                                 └ libshirabe_engine_servo   ── Servo core
```

## Enabling

```toml
shirabe = { version = "0.1", features = ["foreign-engine"] }
```

Then select a foreign backend:

```bash
SHIRABE_BACKEND=firefox shirabe debug --port 3001
SHIRABE_BACKEND=servo   shirabe debug --port 3001
```

## The C ABI a vendor lib exports

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

A few hundred lines of adapter code against this ABI is enough to drive a whole
browser core; everything shirabe exposes over HTTP is built on these five
operations.

## Where the lib comes from

`CdylibEngine::open` looks for `libshirabe_engine_<id>.{so,dylib,dll}` in:

1. `SHIRABE_ENGINE_PATH` — explicit override.
2. next to the current executable.
3. `<cache>/shirabe/engines/<id>/` — where the release-fetch step places
   downloaded copies (a release workflow publishes prebuilt libs to GitHub
   Releases under their own tags).

Until a vendor publishes a lib, selecting Firefox/Servo returns a clear error
pointing at the FFI contract — shirabe never tries to spawn `firefox` as if it
spoke CDP.
