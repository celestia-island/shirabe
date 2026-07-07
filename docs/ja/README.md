<p align="center"><img src="https://raw.githubusercontent.com/celestia-island/shirabe/master/docs/logo.webp" alt="Shirabe" width="240" /></p>

<h1 align="center">Shirabe</h1>

<p align="center"><strong>ヘッドレスブラウザ自動化</strong></p>

<div align="center">

[![License: SySL-1.0](https://img.shields.io/badge/License-SySL--1.0-blue.svg)](https://sysl.celestia.world)
[![GitHub](https://img.shields.io/badge/github-celestia--island%2Fshirabe-blue.svg)](https://github.com/celestia-island/shirabe)
[![Checks](https://img.shields.io/github/actions/workflow/status/celestia-island/shirabe/checks.yml)](https://github.com/celestia-island/shirabe/actions/workflows/checks.yml)
[![Docs](https://img.shields.io/badge/docs-shirabe.docs.celestia.world-blue)](https://shirabe.docs.celestia.world)
[![docs.rs](https://docs.rs/shirabe/badge.svg)](https://docs.rs/shirabe)

</div>

<div align="center">

[English](../en/README.md) ·
[简体中文](../zhs/README.md) ·
[繁體中文](../zht/README.md) ·
**日本語** ·
[한국어](../ko/README.md) ·
[Français](../fr/README.md) ·
[Español](../es/README.md) ·
[Русский](../ru/README.md) ·
[العربية](../ar/README.md)

</div>

## はじめに

shirabe は軽量な Rust ネイティブのブラウザ自動化ライブラリ兼デバッグサーバーです。
Chrome DevTools Protocol を話す任意のブラウザ（Google Chrome・Chromium・Microsoft Edge）
を単一の手書き CDP エンジンで駆動し、それらすべてをコンパクトな HTTP API で公開します。
これは tairitsu パッケージャから抽出され、単独で機能するよう強化されたブラウザ基盤です。

根底にある思想は ONNX Runtime 向けの [ort](https://crates.io/crates/ort) と同じです。
**ブラウザを手作業でインストールする必要は決してありません。** ピン留めされた Chrome
for Testing ビルドがビルド時（または初回利用時）に共有キャッシュへ取得され、透過的に
特定されて CDP 経由で駆動されます。バックエンドの切り替え、ネイティブライブラリの同梱、
ミラーやプロキシ経由でのダウンロードは、すべて環境変数で行えます。

## クイックスタート

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

### ライブラリ

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

## バックエンドとゼロ設定による解決

バックエンドの選択には `SHIRABE_BACKEND=chrome|chromium|edge|firefox|servo|auto`
（デフォルトは `auto`）を用います。**Chromium 系**（Chrome / Chromium / Edge）は
自前の CDP エンジンによってプロセス内で駆動されます。一方、**Firefox** と **Servo**
は異なる経路をとります。これらのコアはブラウザベンダーによってビルドされ、動的
ライブラリとして配布されており、shirabe はそれを薄い C バインディングの FFI 規約
（`foreign-engine` フィーチャー、詳細は[外部エンジン](../en/guides/foreign-engines.md)
を参照）を通じて駆動します。いずれの場合も、shirabe は以下の順序でバックエンドを解決します。

1. **バックエンド固有の上書き** — `CHROME_PATH` / `CHROMIUM_PATH` / `EDGE_PATH`。
2. **ビルド時に埋め込まれたパス** — `SHIRABE_BROWSER_PATH`。`auto-fetch` フィーチャー
   が有効な場合に `build.rs` が Chrome for Testing をビルド中にダウンロードした際に出力されます。
3. **システムバイナリ** — `$PATH` および既知のインストール場所から検索。
4. **実行時フェッチ**（`runtime-fetch` フィーチャー） — ピン留めされたビルドを共有
   キャッシュへダウンロード。

ダウンロード設定（ビルド時・実行時共通）:

| 環境変数 | 用途 |
|-----|---------|
| `SHIRABE_CHROME_VERSION` | ピン留めされた Chrome for Testing のバージョンを上書きします。 |
| `SHIRABE_CHROME_MIRROR` | `storage.googleapis.com` の代わりにミラーからダウンロードします。 |
| `SHIRABE_CHROME_SHA256` | ダウンロードを検証するためのオプションの 16 進数チェックサム。 |
| `SHIRABE_DOWNLOAD_PROXY` | ダウンロードを `http://` / `https://` / `socks5://` プロキシ経由で行います。 |
| `SHIRABE_DOWNLOAD_TIMEOUT_SECS` | リクエストごとのタイムアウト（デフォルト 600）。 |
| `SHIRABE_SKIP_BROWSER_FETCH` | ビルド時および実行時のダウンロードを両方スキップします。 |
| `SHIRABE_BACKEND` | 駆動する Chromium 系バックエンドを指定します。 |

## 製品へのネイティブライブラリ同梱

取得された Chrome ビルド（および皆様のクレート）は、クリーンなコンテナには存在しない
可能性のあるネイティブライブラリに依存しています。shirabe はパッケージャ向けに
2 つのツールを提供します。

- **宣言的バンドル** — 同梱する `.so` / `.dylib` / `.dll` ファイルを
  `SHIRABE_BUNDLE_LIBS`（パス区切りリスト）または `SHIRABE_BUNDLE_MANIFEST`
  （`[[lib]]` テーブルを持つ `bundle.toml`）で指定します。マニフェストの例:

  ```toml
  [[lib]]
  path = "third_party/libfoo.so"
  optional = true
  target_os = "linux"

  [[lib]]
  path = "third_party/foo.dll"
  ```

- **依存関係スキャン** — `shirabe::collect_runtime_deps(exe)` は、バイナリがリンク
  している共有ライブラリを列挙し（`ldd` / `otool -L` / PE インポートスキャン）、
  `shirabe::render_bundle_report(&BundleReport::build(&exe))` はリリーススクリプトが
  バイナリと共にコピーすべきすべてを出力します。

```rust
use shirabe::{BundleReport, render_bundle_report};
let report = BundleReport::build(&backend_exe);
print!("{}", render_bundle_report(&report));
```

## HTTP API

| メソッド | パス | 説明 |
|--------|------|-------------|
| `GET`  | `/health` | サーバーの健全性 |
| `GET`  | `/info` | ブラウザの状態と選択中のバックエンド |
| `POST` | `/navigate` | URL へのナビゲート |
| `POST` | `/click` | 要素のクリック |
| `POST` | `/type` | テキストの入力 |
| `POST` | `/evaluate` | JavaScript の実行 |
| `POST` | `/screenshot` | スクリーンショットの取得 |
| `POST` | `/wait-for-selector` | 要素の待機 |
| `GET`  | `/dom` | DOM のクエリ |
| `GET`  | `/a11y` | アクセシビリティツリー |
| `POST` | `/batch` | バッチ操作 |

…さらに、完全な制御のためのコンソール、ネットワーク、WebSocket のキャプチャエンド
ポイントも備えています。

## 開発

```bash
SHIRABE_SKIP_BROWSER_FETCH=1 cargo clippy --all-targets --all-features -- -D warnings
SHIRABE_SKIP_BROWSER_FETCH=1 cargo test --all-features
```

## ライセンス

SySL-1.0 (Synthetic Source License)。詳細は [LICENSE](https://sysl.celestia.world) を参照してください。
