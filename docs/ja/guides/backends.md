# バックエンドと解決

shirabeは、Chrome DevTools Protocolを話すすべてのブラウザ — Google Chrome、
Chromium、Microsoft Edge — を単一のCDPエンジンで操作します。`SHIRABE_BACKEND`
で選択します：

| Value | Backend |
|-------|---------|
| `chrome` (default in `auto`) | Google Chrome |
| `chromium` | Chromium |
| `edge` | Microsoft Edge |
| `auto` (default) | Try Chrome, then Chromium, then Edge |

## 解決順序

どのバックエンドが選択されても、shirabeは以下の順序で実行可能ファイルを解決します
（[ort](https://crates.io/crates/ort)の依存モデルを反映）：

1. **バックエンド固有の上書き** — `CHROME_PATH` / `CHROMIUM_PATH` / `EDGE_PATH`。
   設定されている場合、そのパスが優先されます。パスが存在しない場合はハードエラーです。
2. **ビルド時埋め込みパス** — `SHIRABE_BROWSER_PATH`。`build.rs`によって生成され、
   `auto-fetch`機能がコンパイル中に固定バージョンのChrome for Testingビルドを
   共有キャッシュにダウンロードした場合に設定されます。
3. **システムバイナリ** — `$PATH`上のバイナリ、および既知のインストール場所
   （`/usr/bin/google-chrome`、`/Applications/Google Chrome.app/...`、
   `C:\Program Files\Google\Chrome\Application\chrome.exe`など）。
4. **実行時フェッチ**（`runtime-fetch`機能） — 初回使用時に固定バージョンの
   Chrome for Testingビルドをキャッシュにダウンロードします。

## ダウンロード設定

フェッチステップは、ビルド時（`build.rs`）と実行時の両方で、以下の環境変数を
参照します：

| Env | Purpose |
|-----|---------|
| `SHIRABE_CHROME_VERSION` | 固定されたChrome for Testingのバージョンを上書きします。 |
| `SHIRABE_CHROME_MIRROR` | デフォルトのGoogleホストではなく、ミラー（GFW対応のものなど）からダウンロードします。 |
| `SHIRABE_CHROME_SHA256` | ビルド時は既定で必須：build.rs が各アーカイブの sha256 を固定し、不一致・欠落時に中止します。実行時も同じ固定表がキャッシュ信頼マーカーを駆動します。固定されていない版・組み合わせは未検証のままで、保証のないツリーの再インストールが失敗した場合はエラーにせず警告付きでそのツリーへ戻ります（回避するには CHROME_PATH を信頼できるビルドに向けてください）。この変数は固定ダイジェストを上書きするだけです。 |
| `SHIRABE_DOWNLOAD_PROXY` | `http://`、`https://`、または`socks5://`プロキシ経由でダウンロードをルーティングします。 |
| `SHIRABE_DOWNLOAD_TIMEOUT_SECS` | リクエストごとのタイムアウト（デフォルト600）。 |
| `SHIRABE_SKIP_BROWSER_FETCH` | ビルド時と実行時の両方のダウンロードをスキップします。 |

> `build.rs`もこれらを読み取るため、下流のクレートはCIにおいて単一の
> `env:`ブロックでツールチェーン全体を固定できます。
