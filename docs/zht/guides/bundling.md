# 捆綁原生函式庫

當你發布基於 shirabe 建構的產品時，通常有兩類原生檔案需要隨二進位檔案一起分發：

1. **瀏覽器後端的執行階段依賴。** 取得的 Chrome for Testing 建構會連結到系統函式庫（`libnss3.so`、`libdbus-1.so`、……），而乾淨的容器中可能沒有這些函式庫。
2. **你自己的原生依賴** — 你的 crate 所連結的 `.so` / `.dylib` / `.dll` 檔案。

shirabe 為打包者提供了一個模組 — `shirabe::bundle` — 來同時處理這兩種情況。

## 宣告要分發的內容

使用 `SHIRABE_BUNDLE_LIBS` 直接列出檔案（路徑分隔符號清單：Unix 上為 `:`，Windows 上為 `;`）：

```bash
SHIRABE_BUNDLE_LIBS="/opt/myapp/libfoo.so:/opt/myapp/libbar.so"
```

或者編寫 `bundle.toml` 清單並透過 `SHIRABE_BUNDLE_MANIFEST` 指向它：

```toml
[[lib]]
path = "third_party/libfoo.so"
optional = true
target_os = "linux"

[[lib]]
path = "third_party/foo.dll"
```

兩種來源透過 `BundleSpec::from_env()` 合併。

## 發現要分發的內容

`collect_runtime_deps(exe)` 掃描二進位檔案以尋找其共享函式庫依賴 — Linux 上使用 `ldd`，macOS 上使用 `otool -L`，Windows 上使用盡力而為的 PE 匯入掃描 — 並傳回每個已記錄的依賴及其解析器找到的位置。

## 整合在一起

`BundleReport::build(&backend_exe)` 將宣告的捆綁包與從已解析的後端可執行檔案中發現的依賴合併，`render_bundle_report(&report)` 將其轉換為發行指令碼可以列印或寫入清單的人類可讀指南：

```rust
use shirabe::{BundleReport, render_bundle_report};

let report = BundleReport::build(&backend_exe);
print!("{}", render_bundle_report(&report));
```

然後，發行指令碼可以將每個 `resolved` 路徑（以及每個已宣告的非可選函式庫）`cp` 到分發目錄中，從而產生一個自包含的產品，該產品可以在未安裝 Chrome 或其系統函式庫的機器上執行。
