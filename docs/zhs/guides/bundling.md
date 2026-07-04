# 捆绑原生库

当你发布基于 shirabe 构建的产品时，通常有两类原生文件需要随二进制文件一起分发：

1. **浏览器后端的运行时依赖。** 获取的 Chrome for Testing 构建会链接到系统库（`libnss3.so`、`libdbus-1.so`、……），而干净的容器中可能没有这些库。
2. **你自己的原生依赖** — 你的 crate 所链接的 `.so` / `.dylib` / `.dll` 文件。

shirabe 为打包者提供了一个模块 — `shirabe::bundle` — 来同时处理这两种情况。

## 声明要分发的内容

使用 `SHIRABE_BUNDLE_LIBS` 直接列出文件（路径分隔符列表：Unix 上为 `:`，Windows 上为 `;`）：

```bash
SHIRABE_BUNDLE_LIBS="/opt/myapp/libfoo.so:/opt/myapp/libbar.so"
```

或者编写 `bundle.toml` 清单并通过 `SHIRABE_BUNDLE_MANIFEST` 指向它：

```toml
[[lib]]
path = "third_party/libfoo.so"
optional = true
target_os = "linux"

[[lib]]
path = "third_party/foo.dll"
```

两种来源通过 `BundleSpec::from_env()` 合并。

## 发现要分发的内容

`collect_runtime_deps(exe)` 扫描二进制文件以查找其共享库依赖 — Linux 上使用 `ldd`，macOS 上使用 `otool -L`，Windows 上使用尽力而为的 PE 导入扫描 — 并返回每个已记录的依赖及其解析器找到的位置。

## 整合在一起

`BundleReport::build(&backend_exe)` 将声明的捆绑包与从已解析的后端可执行文件中发现的依赖合并，`render_bundle_report(&report)` 将其转换为发布脚本可以打印或写入清单的人类可读指南：

```rust
use shirabe::{BundleReport, render_bundle_report};

let report = BundleReport::build(&backend_exe);
print!("{}", render_bundle_report(&report));
```

然后，发布脚本可以将每个 `resolved` 路径（以及每个已声明的非可选库）`cp` 到分发目录中，从而生成一个自包含的产品，该产品可以在未安装 Chrome 或其系统库的机器上运行。
