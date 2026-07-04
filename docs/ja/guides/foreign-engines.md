# 外部エンジン — Firefox & Servo

Chromium ファミリー（Chrome / Chromium / Edge）は、shirabe 独自の CDP エンジン
を通じてプロセス内で駆動されます。**Firefox** と **Servo** は異なる道をたどり
ます：それらのコアは巨大なため、ブラウザベンダー（あるいはそれらのコアをビルド
する人）が固定の C ABI に対して小さなアダプターをコンパイルし、動的ライブラリ
として出荷する方式を取ります — [ort](https://crates.io/crates/ort) が ONNX
Runtime に使っているのと同じモデルです。shirabe は「薄い C バインディング
ラッパー」であり、ベンダーライブラリを dlopen し、汎用的な
[`Engine`](https://shirabe.docs.celestia.world) トレイトを通じて呼び出しを中継
します。

```
your app ── shirabe (CDP engine) ── Chrome / Chromium / Edge   (in-process)
        └─ shirabe (FFI wrapper) ── libshirabe_engine_firefox ── Firefox core
                                 └ libshirabe_engine_servo   ── Servo core
```

## 有効化

```toml
shirabe = { version = "0.1", features = ["foreign-engine"] }
```

次に外部バックエンドを選択します：

```bash
SHIRABE_BACKEND=firefox shirabe debug --port 3001
SHIRABE_BACKEND=servo   shirabe debug --port 3001
```

## ベンダーライブラリがエクスポートする C ABI

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

この ABI に対する数百行のアダプターコードがあれば、ブラウザコア全体を駆動する
のに十分です。shirabe が HTTP 経由で公開するすべての機能は、これら 5 つの操作
の上に構築されています。

## ライブラリの入手先

`CdylibEngine::open` は以下の場所で `libshirabe_engine_<id>.{so,dylib,dll}` を
検索します：

1. `SHIRABE_ENGINE_PATH` — 明示的な上書き。
2. 現在の実行ファイルの隣。
3. `<cache>/shirabe/engines/<id>/` — リリース取得ステップがダウンロードした
   コピーを配置する場所（リリースワークフローがビルド済みライブラリを独自の
   タグで GitHub Releases に公開します）。

ベンダーがライブラリを公開するまでは、Firefox/Servo を選択すると FFI 契約を
指し示す明確なエラーが返ります — shirabe が `firefox` を CDP を話すかのように
起動しようとすることは決してありません。
