# ネイティブライブラリのバンドル

shirabe で構築された製品を出荷する際、通常2種類のネイティブファイルをバイナリと共に同梱する必要があります：

1. **ブラウザバックエンドのランタイム依存関係。** 取得した Chrome for Testing ビルドは、クリーンなコンテナには存在しない可能性のあるシステムライブラリ（`libnss3.so`、`libdbus-1.so`、…）にリンクしています。
2. **独自のネイティブ依存関係** — クレートがリンクする `.so` / `.dylib` / `.dll` ファイル。

shirabe はパッケージャに1つのモジュール — `shirabe::bundle` — を提供し、この両方を処理します。

## 同梱するものを宣言する

`SHIRABE_BUNDLE_LIBS` を使用してファイルをそのまま列挙します（パス区切りリスト：Unix では `:`、Windows では `;`）：

```bash
SHIRABE_BUNDLE_LIBS="/opt/myapp/libfoo.so:/opt/myapp/libbar.so"
```

または `bundle.toml` マニフェストを作成し、`SHIRABE_BUNDLE_MANIFEST` で参照します：

```toml
[[lib]]
path = "third_party/libfoo.so"
optional = true
target_os = "linux"

[[lib]]
path = "third_party/foo.dll"
```

両方のソースは `BundleSpec::from_env()` によってマージされます。

## 同梱するものを検出する

`collect_runtime_deps(exe)` はバイナリをスキャンして共有ライブラリ依存関係を調べます — Linux では `ldd`、macOS では `otool -L`、Windows ではベストエフォートの PE インポートスキャン — そして、記録された各依存関係をリゾルバが見つけた場所と共に返します。

## まとめる

`BundleReport::build(&backend_exe)` は宣言されたバンドルと、解決されたバックエンド実行ファイルから検出された依存関係をマージし、`render_bundle_report(&report)` はそれをリリーススクリプトが表示したりマニフェストに書き込んだりできる人間が読めるガイダンスに変換します：

```rust
use shirabe::{BundleReport, render_bundle_report};

let report = BundleReport::build(&backend_exe);
print!("{}", render_bundle_report(&report));
```

リリーススクリプトは、すべての `resolved` パス（および宣言されたすべての非オプショナルライブラリ）を配布ディレクトリに `cp` することで、Chrome やそのシステムライブラリがインストールされていないマシンでも動作する自己完結型の製品を生成できます。
