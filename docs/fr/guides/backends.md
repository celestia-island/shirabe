# Backends et résolution

shirabe pilote tout navigateur parlant le protocole Chrome DevTools — Google
Chrome, Chromium, Microsoft Edge — via un seul moteur CDP. Choisissez-en un avec
`SHIRABE_BACKEND` :

| Value | Backend |
|-------|---------|
| `chrome` (default in `auto`) | Google Chrome |
| `chromium` | Chromium |
| `edge` | Microsoft Edge |
| `auto` (default) | Try Chrome, then Chromium, then Edge |

## Ordre de résolution

Quel que soit le backend choisi, shirabe résout un exécutable dans cet ordre
(reflétant le modèle de dépendances d'[ort](https://crates.io/crates/ort)) :

1. **Surcharge spécifique au backend** — `CHROME_PATH` / `CHROMIUM_PATH` / `EDGE_PATH`.
   S'il est défini, le chemin fait autorité ; un chemin manquant est une erreur
   fatale.
2. **Chemin intégré à la compilation** — `SHIRABE_BROWSER_PATH`, émis par
   `build.rs` lorsque la fonctionnalité `auto-fetch` télécharge la version
   épinglée de Chrome for Testing dans le cache partagé pendant la compilation.
3. **Binaire système** dans `$PATH` ainsi qu'un ensemble d'emplacements
   d'installation connus (`/usr/bin/google-chrome`,
   `/Applications/Google Chrome.app/...`,
   `C:\Program Files\Google\Chrome\Application\chrome.exe`, …).
4. **Récupération à l'exécution** (fonctionnalité `runtime-fetch`) —
   téléchargement de la version épinglée de Chrome for Testing dans le cache
   lors de la première utilisation.

## Paramètres de téléchargement

L'étape de récupération respecte ces variables d'environnement, à la fois à la
compilation (`build.rs`) et à l'exécution :

| Env | Purpose |
|-----|---------|
| `SHIRABE_CHROME_VERSION` | Remplace la version épinglée de Chrome for Testing. |
| `SHIRABE_CHROME_MIRROR` | Télécharge depuis un miroir (par ex. compatible GFW) au lieu de l'hôte Google par défaut. |
| `SHIRABE_CHROME_SHA256` | Somme de contrôle hexadécimale optionnelle ; le téléchargement est vérifié par rapport à celle-ci. |
| `SHIRABE_DOWNLOAD_PROXY` | Achemine le téléchargement via un proxy `http://`, `https://` ou `socks5://`. |
| `SHIRABE_DOWNLOAD_TIMEOUT_SECS` | Délai d'expiration par requête (par défaut 600). |
| `SHIRABE_SKIP_BROWSER_FETCH` | Ignore les téléchargements à la compilation et à l'exécution. |

> Puisque `build.rs` les lit également, un crate dépendant peut verrouiller
> toute la chaîne d'outils en CI avec un seul bloc `env:`.
