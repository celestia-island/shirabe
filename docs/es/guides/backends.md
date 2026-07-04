# Backends y resolución

shirabe controla cualquier navegador que hable el protocolo Chrome DevTools —
Google Chrome, Chromium, Microsoft Edge — a través de un único motor CDP.
Selecciona uno con `SHIRABE_BACKEND`:

| Value | Backend |
|-------|---------|
| `chrome` (default in `auto`) | Google Chrome |
| `chromium` | Chromium |
| `edge` | Microsoft Edge |
| `auto` (default) | Try Chrome, then Chromium, then Edge |

## Orden de resolución

Sea cual sea el backend elegido, shirabe resuelve un ejecutable en este orden
(reflejando el modelo de dependencias de [ort](https://crates.io/crates/ort)):

1. **Sobrescritura específica del backend** — `CHROME_PATH` / `CHROMIUM_PATH` / `EDGE_PATH`.
   Si se establece, la ruta es autoritativa; una ruta faltante es un error grave.
2. **Ruta incrustada en tiempo de compilación** — `SHIRABE_BROWSER_PATH`, emitida
   por `build.rs` cuando la funcionalidad `auto-fetch` descarga la compilación
   fijada de Chrome for Testing en la caché compartida durante la compilación.
3. **Binario del sistema** en `$PATH` más un conjunto de ubicaciones de
   instalación conocidas (`/usr/bin/google-chrome`,
   `/Applications/Google Chrome.app/...`,
   `C:\Program Files\Google\Chrome\Application\chrome.exe`, …).
4. **Obtención en tiempo de ejecución** (funcionalidad `runtime-fetch`) —
   descarga la compilación fijada de Chrome for Testing en la caché en el
   primer uso.

## Controles de descarga

El paso de obtención respeta estas variables de entorno, tanto en tiempo de
compilación (`build.rs`) como en tiempo de ejecución:

| Env | Purpose |
|-----|---------|
| `SHIRABE_CHROME_VERSION` | Sobrescribe la versión fijada de Chrome for Testing. |
| `SHIRABE_CHROME_MIRROR` | Descarga desde un espejo (p. ej., uno compatible con el GFW) en lugar del host predeterminado de Google. |
| `SHIRABE_CHROME_SHA256` | Suma de verificación hexadecimal opcional; la descarga se verifica contra ella. |
| `SHIRABE_DOWNLOAD_PROXY` | Enruta la descarga a través de un proxy `http://`, `https://` o `socks5://`. |
| `SHIRABE_DOWNLOAD_TIMEOUT_SECS` | Tiempo de espera por solicitud (predeterminado 600). |
| `SHIRABE_SKIP_BROWSER_FETCH` | Omite las descargas tanto en tiempo de compilación como en tiempo de ejecución. |

> Dado que `build.rs` también lee estas variables, un crate dependiente puede
> fijar toda la cadena de herramientas en CI con un solo bloque `env:`.
