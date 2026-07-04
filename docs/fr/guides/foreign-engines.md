# Moteurs externes — Firefox & Servo

La famille Chromium (Chrome / Chromium / Edge) est pilotée en cours de processus
via le moteur CDP propre à shirabe. **Firefox** et **Servo** empruntent un chemin
différent : leurs cœurs sont énormes, nous laissons donc les éditeurs de
navigateurs (ou quiconque compile ces cœurs) compiler un petit adaptateur contre
une C ABI fixe et le livrer sous forme de bibliothèque dynamique — le même modèle
qu'utilise [ort](https://crates.io/crates/ort) pour ONNX Runtime. shirabe est
l'« enveloppe fine de liaison C » : il ouvre la bibliothèque du fournisseur via
dlopen et achemine les appels via un trait générique
[`Engine`](https://shirabe.docs.celestia.world).

```
your app ── shirabe (CDP engine) ── Chrome / Chromium / Edge   (in-process)
        └─ shirabe (FFI wrapper) ── libshirabe_engine_firefox ── Firefox core
                                 └ libshirabe_engine_servo   ── Servo core
```

## Activation

```toml
shirabe = { version = "0.1", features = ["foreign-engine"] }
```

Puis sélectionnez un backend externe :

```bash
SHIRABE_BACKEND=firefox shirabe debug --port 3001
SHIRABE_BACKEND=servo   shirabe debug --port 3001
```

## La C ABI qu'exporte une bibliothèque fournisseur

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

Quelques centaines de lignes de code adaptateur contre cette ABI suffisent à
piloter un cœur de navigateur complet ; tout ce que shirabe expose via HTTP est
construit sur ces cinq opérations.

## D'où provient la bibliothèque

`CdylibEngine::open` recherche `libshirabe_engine_<id>.{so,dylib,dll}` dans :

1. `SHIRABE_ENGINE_PATH` — remplacement explicite.
2. à côté de l'exécutable actuel.
3. `<cache>/shirabe/engines/<id>/` — où l'étape de récupération de version place
   les copies téléchargées (un workflow de publication envoie des bibliothèques
   précompilées sur GitHub Releases sous leurs propres tags).

Tant qu'un fournisseur n'a pas publié de bibliothèque, sélectionner Firefox/Servo
renvoie une erreur claire pointant vers le contrat FFI — shirabe n'essaie jamais
de lancer `firefox` comme s'il parlait CDP.
