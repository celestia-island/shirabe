<p align="center"><img src="https://raw.githubusercontent.com/celestia-island/shirabe/master/docs/logo.webp" alt="shirabe" width="240" /></p>

<h1 align="center">shirabe</h1>

<p align="center"><strong>Automatisation de navigateur sans tête</strong></p>

<div align="center">

[![License: SySL-1.0](https://img.shields.io/badge/License-SySL--1.0-blue.svg)](https://sysl.celestia.world)
[![Checks](https://img.shields.io/github/actions/workflow/status/celestia-island/shirabe/checks.yml)](https://github.com/celestia-island/shirabe/actions/workflows/checks.yml)
[![Docs](https://img.shields.io/badge/docs-shirabe.docs.celestia.world-blue)](https://shirabe.docs.celestia.world)

</div>

<div align="center">

[English](../en/README.md) ·
[简体中文](../zhs/README.md) ·
[繁體中文](../zht/README.md) ·
[日本語](../ja/README.md) ·
[한국어](../ko/README.md) ·
**Français** ·
[Español](../es/README.md) ·
[Русский](../ru/README.md) ·
[العربية](../ar/README.md)

</div>

## Introduction

shirabe est une bibliothèque d'automatisation de navigateur légère et native
Rust, ainsi qu'un serveur de débogage. Il pilote tout navigateur qui parle le
Chrome DevTools Protocol — Google Chrome, Chromium, Microsoft Edge — via un
moteur CDP écrit à la main, et expose le tout à travers une petite API HTTP.
C'est le socle navigateur extrait de l'empaqueteur tairitsu, durci pour
fonctionner de manière autonome.

L'idée directrice est la même que celle d'[ort](https://crates.io/crates/ort)
pour ONNX Runtime : **vous ne devriez jamais avoir à installer un navigateur
manuellement.** Une version épinglée de Chrome for Testing est récupérée dans
un cache partagé à la compilation (ou à la première utilisation), localisée de
manière transparente, et pilotée via CDP. Épinglez un backend différent, livrez
des bibliothèques natives avec votre produit, faites transiter le
téléchargement par un miroir ou un proxy — le tout via des variables
d'environnement.

## Démarrage rapide

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

### Bibliothèque

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

## Backends et résolution sans configuration

Choisissez un backend avec `SHIRABE_BACKEND=chrome|chromium|edge|firefox|servo|auto`
(par défaut `auto`). La **famille Chromium** (Chrome / Chromium / Edge) est
pilotée en processus via notre propre moteur CDP ; **Firefox** et **Servo**
empruntent un chemin différent — leurs cœurs sont compilés par les éditeurs de
navigateurs et livrés sous forme de bibliothèques dynamiques, que shirabe
pilote via un contrat FFI en liaison C (la fonctionnalité `foreign-engine`,
voir [Moteurs externes](../en/guides/foreign-engines.md)). Quelle que soit
l'option choisie, shirabe la résout dans cet ordre :

1. **Surcharge spécifique au backend** — `CHROME_PATH` / `CHROMIUM_PATH` / `EDGE_PATH`.
2. **Chemin intégré à la compilation** — `SHIRABE_BROWSER_PATH`, émis par
   `build.rs` lorsque la fonctionnalité `auto-fetch` télécharge Chrome for
   Testing pendant la compilation.
3. **Binaire système** présent dans `$PATH` et les répertoires d'installation
   courants.
4. **Téléchargement à l'exécution** (fonctionnalité `runtime-fetch`) —
   télécharge la version épinglée dans le cache partagé.

Paramètres de téléchargement (compilation et exécution) :

| Variable d'env | Rôle |
|-----|---------|
| `SHIRABE_CHROME_VERSION` | Remplace la version épinglée de Chrome for Testing. |
| `SHIRABE_CHROME_MIRROR` | Télécharge depuis un miroir au lieu de `storage.googleapis.com`. |
| `SHIRABE_CHROME_SHA256` | Somme de contrôle hexadécimale optionnelle pour vérifier le téléchargement. |
| `SHIRABE_DOWNLOAD_PROXY` | Fait transiter le téléchargement via un proxy `http://` / `https://` / `socks5://`. |
| `SHIRABE_DOWNLOAD_TIMEOUT_SECS` | Délai d'expiration par requête (par défaut 600). |
| `SHIRABE_SKIP_BROWSER_FETCH` | Ignore les téléchargements à la compilation et à l'exécution. |
| `SHIRABE_BACKEND` | Quel backend de la famille Chromium piloter. |

## Livrer des bibliothèques natives avec votre produit

Une version téléchargée de Chrome (et votre propre crate) dépend de
bibliothèques natives qu'un conteneur vierge peut ne pas avoir. shirabe offre
aux empaqueteurs deux outils :

- **Lot déclaratif** — listez les fichiers `.so` / `.dylib` / `.dll` à livrer
  via `SHIRABE_BUNDLE_LIBS` (liste séparée par le séparateur de chemin) ou
  `SHIRABE_BUNDLE_MANIFEST` (un `bundle.toml` de tables `[[lib]]`). Exemple de
  manifeste :

  ```toml
  [[lib]]
  path = "third_party/libfoo.so"
  optional = true
  target_os = "linux"

  [[lib]]
  path = "third_party/foo.dll"
  ```

- **Analyse des dépendances** — `shirabe::collect_runtime_deps(exe)` énumère
  les bibliothèques partagées auxquelles un binaire est lié (`ldd` /
  `otool -L` / une analyse des imports PE), et
  `shirabe::render_bundle_report(&BundleReport::build(&exe))` affiche tout ce
  qu'un script de publication devrait copier avec le binaire.

```rust
use shirabe::{BundleReport, render_bundle_report};
let report = BundleReport::build(&backend_exe);
print!("{}", render_bundle_report(&report));
```

## API HTTP

| Méthode | Chemin | Description |
|--------|------|-------------|
| `GET`  | `/health` | Santé du serveur |
| `GET`  | `/info` | Statut du navigateur + backend sélectionné |
| `POST` | `/navigate` | Naviguer vers une URL |
| `POST` | `/click` | Cliquer sur un élément |
| `POST` | `/type` | Saisir du texte |
| `POST` | `/evaluate` | Exécuter du JavaScript |
| `POST` | `/screenshot` | Capturer une capture d'écran |
| `POST` | `/wait-for-selector` | Attendre un élément |
| `GET`  | `/dom` | Interroger le DOM |
| `GET`  | `/a11y` | Arbre d'accessibilité |
| `POST` | `/batch` | Opérations par lot |

…ainsi que des points de terminaison de capture console, réseau et websocket
pour un contrôle total.

## Développement

```bash
SHIRABE_SKIP_BROWSER_FETCH=1 cargo clippy --all-targets --all-features -- -D warnings
SHIRABE_SKIP_BROWSER_FETCH=1 cargo test --all-features
```

## Licence

SySL-1.0 (Synthetic Source License). Voir [LICENSE](https://sysl.celestia.world).
