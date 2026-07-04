# Motores externos — Firefox & Servo

La familia Chromium (Chrome / Chromium / Edge) se controla dentro del proceso
mediante el motor CDP propio de shirabe. **Firefox** y **Servo** toman un camino
distinto: sus núcleos son enormes, así que dejamos que los proveedores de
navegadores (o cualquiera que compile esos núcleos) compilen un pequeño adaptador
contra una C ABI fija y lo distribuyan como biblioteca dinámica — el mismo modelo
que usa [ort](https://crates.io/crates/ort) para ONNX Runtime. shirabe es el
"envoltorio delgado de bindings C": abre la biblioteca del proveedor con dlopen y
enruta las llamadas a través de un trait genérico
[`Engine`](https://shirabe.docs.celestia.world).

```
your app ── shirabe (CDP engine) ── Chrome / Chromium / Edge   (in-process)
        └─ shirabe (FFI wrapper) ── libshirabe_engine_firefox ── Firefox core
                                 └ libshirabe_engine_servo   ── Servo core
```

## Activación

```toml
shirabe = { version = "0.1", features = ["foreign-engine"] }
```

Luego selecciona un backend externo:

```bash
SHIRABE_BACKEND=firefox shirabe debug --port 3001
SHIRABE_BACKEND=servo   shirabe debug --port 3001
```

## La C ABI que exporta una biblioteca de proveedor

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

Unos pocos cientos de líneas de código adaptador contra esta ABI son suficientes
para controlar un núcleo de navegador completo; todo lo que shirabe expone por
HTTP está construido sobre estas cinco operaciones.

## De dónde proviene la biblioteca

`CdylibEngine::open` busca `libshirabe_engine_<id>.{so,dylib,dll}` en:

1. `SHIRABE_ENGINE_PATH` — sustitución explícita.
2. junto al ejecutable actual.
3. `<cache>/shirabe/engines/<id>/` — donde el paso de obtención de versión coloca
   las copias descargadas (un flujo de trabajo de publicación sube bibliotecas
   precompiladas a GitHub Releases bajo sus propias etiquetas).

Hasta que un proveedor publique una biblioteca, seleccionar Firefox/Servo devuelve
un error claro que apunta al contrato FFI — shirabe nunca intenta lanzar `firefox`
como si hablara CDP.
