# Empaquetage de Bibliothèques Natives

Lorsque vous livrez un produit construit avec shirabe, deux catégories de fichiers natifs doivent généralement accompagner le binaire :

1. **Les dépendances d'exécution du backend du navigateur.** Une version de Chrome for Testing récupérée se lie à des bibliothèques système (`libnss3.so`, `libdbus-1.so`, …) qu'un conteneur propre peut ne pas avoir.
2. **Vos propres dépendances natives** — fichiers `.so` / `.dylib` / `.dll` contre lesquels votre crate se lie.

shirabe offre aux empaqueteurs un module — `shirabe::bundle` — pour gérer les deux.

## Déclarer ce qu'il faut livrer

Listez les fichiers textuellement avec `SHIRABE_BUNDLE_LIBS` (liste séparée par le délimiteur de chemin : `:` sur Unix, `;` sur Windows) :

```bash
SHIRABE_BUNDLE_LIBS="/opt/myapp/libfoo.so:/opt/myapp/libbar.so"
```

Ou écrivez un manifeste `bundle.toml` et référencez-le avec `SHIRABE_BUNDLE_MANIFEST` :

```toml
[[lib]]
path = "third_party/libfoo.so"
optional = true
target_os = "linux"

[[lib]]
path = "third_party/foo.dll"
```

Les deux sources sont fusionnées par `BundleSpec::from_env()`.

## Découvrir ce qu'il faut livrer

`collect_runtime_deps(exe)` analyse un binaire pour trouver ses dépendances de bibliothèques partagées — `ldd` sur Linux, `otool -L` sur macOS, une analyse d'importation PE au mieux sur Windows — et retourne chaque dépendance enregistrée avec l'emplacement où le résolveur l'a trouvée.

## Assembler le tout

`BundleReport::build(&backend_exe)` fusionne le paquet déclaré avec les dépendances découvertes à partir de l'exécutable backend résolu, et `render_bundle_report(&report)` le transforme en guide lisible qu'un script de publication peut afficher ou écrire dans un manifeste :

```rust
use shirabe::{BundleReport, render_bundle_report};

let report = BundleReport::build(&backend_exe);
print!("{}", render_bundle_report(&report));
```

Un script de publication peut ensuite copier chaque chemin `resolved` (et chaque bibliothèque déclarée non optionnelle) dans le répertoire de distribution, produisant un produit autonome qui s'exécute sur une machine sans Chrome ni ses bibliothèques système installées.
