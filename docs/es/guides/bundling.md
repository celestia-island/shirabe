# Empaquetado de Bibliotecas Nativas

Cuando distribuyes un producto creado con shirabe, normalmente dos clases de archivos nativos deben acompañar al binario:

1. **Las dependencias de tiempo de ejecución del backend del navegador.** Una compilación de Chrome for Testing obtenida se enlaza con bibliotecas del sistema (`libnss3.so`, `libdbus-1.so`, …) que un contenedor limpio puede no tener.
2. **Tus propias dependencias nativas** — archivos `.so` / `.dylib` / `.dll` contra los que enlaza tu crate.

shirabe proporciona un módulo — `shirabe::bundle` — para manejar ambos casos.

## Declarar qué distribuir

Enumera los archivos literalmente con `SHIRABE_BUNDLE_LIBS` (lista separada por delimitador de ruta: `:` en Unix, `;` en Windows):

```bash
SHIRABE_BUNDLE_LIBS="/opt/myapp/libfoo.so:/opt/myapp/libbar.so"
```

O escribe un manifiesto `bundle.toml` y apúntalo con `SHIRABE_BUNDLE_MANIFEST`:

```toml
[[lib]]
path = "third_party/libfoo.so"
optional = true
target_os = "linux"

[[lib]]
path = "third_party/foo.dll"
```

Ambas fuentes se fusionan mediante `BundleSpec::from_env()`.

## Descubrir qué distribuir

`collect_runtime_deps(exe)` escanea un binario en busca de sus dependencias de bibliotecas compartidas — `ldd` en Linux, `otool -L` en macOS, un escaneo de importación PE de mejor esfuerzo en Windows — y devuelve cada dependencia registrada con la ubicación donde el resolvedor la encontró.

## Unirlo todo

`BundleReport::build(&backend_exe)` fusiona el paquete declarado con las dependencias descubiertas del ejecutable de backend resuelto, y `render_bundle_report(&report)` lo convierte en una guía legible que un script de lanzamiento puede imprimir o escribir en un manifiesto:

```rust
use shirabe::{BundleReport, render_bundle_report};

let report = BundleReport::build(&backend_exe);
print!("{}", render_bundle_report(&report));
```

Un script de lanzamiento puede entonces copiar cada ruta `resolved` (y cada biblioteca declarada no opcional) al directorio de distribución, produciendo un producto autónomo que se ejecuta en una máquina sin Chrome ni sus bibliotecas de sistema instaladas.
