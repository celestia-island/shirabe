# Bundling Native Libraries

When you ship a product built on shirabe, two classes of native file usually
need to travel next to the binary:

1. **The browser backend's runtime dependencies.** A fetched Chrome for Testing
   build links against system libraries (`libnss3.so`, `libdbus-1.so`, …) that
   a clean container may not have.
2. **Your own native dependencies** — `.so` / `.dylib` / `.dll` files your
   crate links against.

shirabe gives packagers one module — `shirabe::bundle` — to handle both.

## Declare what to ship

List files verbatim with `SHIRABE_BUNDLE_LIBS` (path-sep list: `:` on Unix,
`;` on Windows):

```bash
SHIRABE_BUNDLE_LIBS="/opt/myapp/libfoo.so:/opt/myapp/libbar.so"
```

Or write a `bundle.toml` manifest and point at it with
`SHIRABE_BUNDLE_MANIFEST`:

```toml
[[lib]]
path = "third_party/libfoo.so"
optional = true
target_os = "linux"

[[lib]]
path = "third_party/foo.dll"
```

Both sources are merged by `BundleSpec::from_env()`.

## Discover what to ship

`collect_runtime_deps(exe)` scans a binary for its shared-library dependencies
— `ldd` on Linux, `otool -L` on macOS, a best-effort PE import scan on Windows
— and returns each recorded dependency with where the resolver found it.

## Put it together

`BundleReport::build(&backend_exe)` merges the declared bundle with the deps
discovered from the resolved backend executable, and
`render_bundle_report(&report)` turns it into the human-readable guidance a
release script can print or write to a manifest:

```rust
use shirabe::{BundleReport, render_bundle_report};

let report = BundleReport::build(&backend_exe);
print!("{}", render_bundle_report(&report));
```

A release script can then `cp` every `resolved` path (and every declared,
non-optional lib) into the distribution directory, producing a self-contained
product that runs on a machine without Chrome or its system libraries
installed.
