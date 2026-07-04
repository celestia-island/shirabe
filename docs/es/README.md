<p align="center"><img src="https://raw.githubusercontent.com/celestia-island/shirabe/master/docs/logo.webp" alt="shirabe" width="240" /></p>

<h1 align="center">shirabe</h1>

<p align="center"><strong>Automatización de navegador sin cabeza</strong></p>

<div align="center">

[![License: SySL-1.0](https://img.shields.io/badge/License-SySL--1.0-blue.svg)](../../LICENSE)
[![Checks](https://img.shields.io/github/actions/workflow/status/celestia-island/shirabe/checks.yml)](https://github.com/celestia-island/shirabe/actions/workflows/checks.yml)
[![Docs](https://img.shields.io/badge/docs-shirabe.docs.celestia.world-blue)](https://shirabe.docs.celestia.world)

</div>

<div align="center">

[English](../en/README.md) ·
[简体中文](../zhs/README.md) ·
[繁體中文](../zht/README.md) ·
[日本語](../ja/README.md) ·
[한국어](../ko/README.md) ·
[Français](../fr/README.md) ·
**Español** ·
[Русский](../ru/README.md) ·
[العربية](../ar/README.md)

</div>

## Introducción

shirabe es una librería ligera y nativa de Rust para automatización de
navegadores, además de un servidor de depuración. Controla cualquier navegador
que hable el protocolo Chrome DevTools — Google Chrome, Chromium, Microsoft
Edge — mediante un motor CDP escrito a mano, y expone todo a través de una
pequeña API HTTP. Es la base de navegador extraída del empaquetador tairitsu,
reforzada para funcionar por sí sola.

La filosofía es la misma que la de [ort](https://crates.io/crates/ort) para
ONNX Runtime: **nunca deberías tener que instalar un navegador a mano.** Una
versión fijada de Chrome for Testing se descarga a una caché compartida (al
compilar o en el primer uso), se localiza de forma transparente y se controla
mediante CDP. Fija un backend distinto, distribuye librerías nativas con tu
producto, redirige la descarga a través de un mirror o proxy — todo desde
variables de entorno.

## Inicio rápido

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

### Librería

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

## Backends y resolución sin configuración

Elige un backend con `SHIRABE_BACKEND=chrome|chromium|edge|firefox|servo|auto`
(por defecto `auto`). La **familia Chromium** (Chrome / Chromium / Edge) se
controla en proceso mediante nuestro propio motor CDP; **Firefox** y **Servo**
toman un camino distinto — sus núcleos son compilados por los proveedores de
cada navegador y distribuidos como librerías dinámicas, que shirabe controla a
través de una fina capa FFI con contratos en C (la funcionalidad
`foreign-engine`, consulta
[Motores externos](../en/guides/foreign-engines.md)). Sea cual sea la opción
elegida, shirabe lo resuelve en este orden:

1. **Sobrescritura específica del backend** — `CHROME_PATH` / `CHROMIUM_PATH` / `EDGE_PATH`.
2. **Ruta fijada en compilación** — `SHIRABE_BROWSER_PATH`, emitida por `build.rs`
   cuando la funcionalidad `auto-fetch` descarga Chrome for Testing durante la compilación.
3. **Binario del sistema** en `$PATH` y ubicaciones de instalación conocidas.
4. **Descarga en tiempo de ejecución** (funcionalidad `runtime-fetch`) — descarga
   la versión fijada en la caché compartida.

Controles de descarga (tanto en compilación como en ejecución):

| Variable de entorno | Propósito |
|---------------------|-----------|
| `SHIRABE_CHROME_VERSION` | Sobrescribe la versión fijada de Chrome for Testing. |
| `SHIRABE_CHROME_MIRROR` | Descarga desde un mirror en lugar de `storage.googleapis.com`. |
| `SHIRABE_CHROME_SHA256` | Checksum hexadecimal opcional para verificar la descarga. |
| `SHIRABE_DOWNLOAD_PROXY` | Redirige la descarga a través de un proxy `http://` / `https://` / `socks5://`. |
| `SHIRABE_DOWNLOAD_TIMEOUT_SECS` | Tiempo de espera por petición (por defecto 600). |
| `SHIRABE_SKIP_BROWSER_FETCH` | Omite las descargas tanto en compilación como en ejecución. |
| `SHIRABE_BACKEND` | Qué backend de la familia Chromium controlar. |

## Distribuir librerías nativas con tu producto

Una compilación de Chrome descargada (y tu propio crate) dependen de librerías
nativas que un contenedor limpio puede no tener. shirabe ofrece dos herramientas
a los empaquetadores:

- **Paquete declarativo** — lista los archivos `.so` / `.dylib` / `.dll` a
  distribuir mediante `SHIRABE_BUNDLE_LIBS` (lista separada por el separador de
  ruta del sistema) o `SHIRABE_BUNDLE_MANIFEST` (un `bundle.toml` con tablas
  `[[lib]]`). Ejemplo de manifiesto:

  ```toml
  [[lib]]
  path = "third_party/libfoo.so"
  optional = true
  target_os = "linux"

  [[lib]]
  path = "third_party/foo.dll"
  ```

- **Análisis de dependencias** — `shirabe::collect_runtime_deps(exe)` enumera las
  librerías compartidas de las que depende un binario (`ldd` / `otool -L` / un
  análisis de importaciones PE), y `shirabe::render_bundle_report(&BundleReport::build(&exe))`
  imprime todo lo que un script de publicación debería copiar junto al binario.

```rust
use shirabe::{BundleReport, render_bundle_report};
let report = BundleReport::build(&backend_exe);
print!("{}", render_bundle_report(&report));
```

## API HTTP

| Método | Ruta | Descripción |
|--------|------|-------------|
| `GET`  | `/health` | Estado del servidor |
| `GET`  | `/info` | Estado del navegador + backend seleccionado |
| `POST` | `/navigate` | Navegar a una URL |
| `POST` | `/click` | Hacer clic en un elemento |
| `POST` | `/type` | Escribir texto |
| `POST` | `/evaluate` | Ejecutar JavaScript |
| `POST` | `/screenshot` | Capturar una captura de pantalla |
| `POST` | `/wait-for-selector` | Esperar a que aparezca un elemento |
| `GET`  | `/dom` | Consultar el DOM |
| `GET`  | `/a11y` | Árbol de accesibilidad |
| `POST` | `/batch` | Operaciones por lotes |

…más endpoints de consola, red y captura por websocket para un control total.

## Desarrollo

```bash
SHIRABE_SKIP_BROWSER_FETCH=1 cargo clippy --all-targets --all-features -- -D warnings
SHIRABE_SKIP_BROWSER_FETCH=1 cargo test --all-features
```

## Licencia

SySL-1.0 (Synthetic Source License). Consulta [LICENSE](../../LICENSE).
