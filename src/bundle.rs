//! Native-library bundling guidance.
//!
//! When you ship a product built on shirabe, two classes of native files usually
//! need to travel next to the binary:
//!
//! 1. **The browser backend's own runtime dependencies.** A fetched Chrome for
//!    Testing build depends on a set of system libraries (`libnss3.so`,
//!    `libdbus-1.so`, … on Linux; `msedgehtml`-style DLLs on older Edge;
//!    framework dylibs on macOS). On a clean container they are often absent.
//! 2. **Your own native dependencies** — `.so` / `.dylib` / `.dll` files your
//!    crate links against.
//!
//! This module gives packagers a declarative way to say "ship these, too" and a
//! scanner that enumerates the first class automatically:
//!
//! - [`BundleSpec`] is a list of [`NativeLib`] entries, populated from the
//!   `SHIRABE_BUNDLE_LIBS` env var (`:`-separated on Unix, `;` on Windows) or a
//!   `bundle.toml` manifest pointed at by `SHIRABE_BUNDLE_MANIFEST`.
//! - [`collect_runtime_deps`] scans a binary (the resolved backend executable,
//!   or your own product) for its shared-library dependencies and returns the
//!   paths a packager should copy alongside it.
//!
//! Both feed [`BundleReport`], which [`render`] turns into the human-readable
//! guidance a release script can print or write to a manifest.

use std::path::{Path, PathBuf};

/// A native library the user wants shipped next to their product.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeLib {
    /// Absolute or manifest-relative path to the file.
    pub path: PathBuf,
    /// If true, a missing file is a warning rather than an error.
    pub optional: bool,
    /// Restrict the lib to a target OS (`linux`, `windows`, `macos`). `None`
    /// means "everywhere".
    pub target_os: Option<String>,
}

/// Declarative bundle of native libraries to ship.
#[derive(Debug, Clone, Default)]
pub struct BundleSpec {
    pub libs: Vec<NativeLib>,
}

impl BundleSpec {
    /// Load a bundle from the environment.
    ///
    /// - `SHIRABE_BUNDLE_LIBS`: path-sep list (`:` / `;`) of files to ship
    ///   verbatim.
    /// - `SHIRABE_BUNDLE_MANIFEST`: path to a `bundle.toml` of `[[lib]]`
    ///   tables (`path = "…"`, `optional = true`, `target_os = "linux"`).
    ///
    /// Both may be set; entries are merged. Missing sources are silently
    /// ignored so this is safe to call in any environment.
    pub fn from_env() -> Self {
        let mut libs = Vec::new();

        if let Ok(raw) = std::env::var("SHIRABE_BUNDLE_LIBS") {
            let sep = if cfg!(windows) { ';' } else { ':' };
            for entry in raw.split(sep) {
                let entry = entry.trim();
                if entry.is_empty() {
                    continue;
                }
                libs.push(NativeLib {
                    path: PathBuf::from(entry),
                    optional: false,
                    target_os: None,
                });
            }
        }

        if let Ok(manifest) = std::env::var("SHIRABE_BUNDLE_MANIFEST") {
            if let Ok(text) = std::fs::read_to_string(&manifest) {
                libs.extend(parse_manifest(&text));
            }
        }

        BundleSpec { libs }
    }
}

/// Minimal `bundle.toml` parser: `[[lib]]` tables with `path` (required),
/// `optional` (bool, default false), `target_os` (string, optional).
fn parse_manifest(text: &str) -> Vec<NativeLib> {
    let mut out = Vec::new();
    let mut cur: Option<NativeLib> = None;
    for line in text.lines() {
        let line = line.trim();
        if line == "[[lib]]" {
            if let Some(lib) = cur.take() {
                out.push(lib);
            }
            cur = Some(NativeLib {
                path: PathBuf::new(),
                optional: false,
                target_os: None,
            });
            continue;
        }
        let Some((key, val)) = line.split_once('=') else {
            continue;
        };
        let (key, val) = (key.trim(), val.trim());
        let Some(lib) = cur.as_mut() else {
            continue;
        };
        let val = val.trim_matches('"').trim_matches('\'');
        match key {
            "path" => lib.path = PathBuf::from(val),
            "optional" => lib.optional = val == "true",
            "target_os" => lib.target_os = Some(val.to_string()),
            _ => {}
        }
    }
    if let Some(lib) = cur.take() {
        if !lib.path.as_os_str().is_empty() {
            out.push(lib);
        }
    }
    out
}

/// A dependency discovered by scanning a binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dep {
    /// The name as the linker recorded it (e.g. `libnss3.so`).
    pub name: String,
    /// Where the resolver found it, if at all.
    pub resolved: Option<PathBuf>,
}

/// Scan `binary` for its shared-library dependencies.
///
/// Uses `ldd` on Linux, `otool -L` on macOS and a best-effort PE import scan on
/// Windows. Returns one [`Dep`] per recorded dependency. Best-effort: any tool
/// failure yields an empty list rather than an error, so this is safe to call
/// from a release script that should never block the build.
pub fn collect_runtime_deps(binary: &Path) -> Vec<Dep> {
    #[cfg(target_os = "linux")]
    let deps = collect_ldd(binary);
    #[cfg(target_os = "macos")]
    let deps = collect_otool(binary);
    #[cfg(target_os = "windows")]
    let deps = collect_pe_imports(binary);
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    let deps = {
        let _ = binary;
        Vec::new()
    };
    deps
}

#[cfg(target_os = "linux")]
fn collect_ldd(binary: &Path) -> Vec<Dep> {
    let output = match std::process::Command::new("ldd").arg(binary).output() {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    parse_ldd(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(target_os = "linux")]
fn parse_ldd(stdout: &str) -> Vec<Dep> {
    let mut deps = Vec::new();
    for line in stdout.lines().map(|l| l.trim()) {
        if line.is_empty() {
            continue;
        }
        // Shapes:
        //   `libnss3.so => /usr/lib/x86_64-linux-gnu/libnss3.so (0x...)`
        //   `/lib64/ld-linux-x86-64.so.2 (0x...)`
        //   `linux-vdso.so.1 =>  (0x...)`
        if let Some((name, rest)) = line.split_once(" => ") {
            let name = name.trim().to_string();
            // The remainder is `<path> (<addr>)` — or just `(<addr>)` when the
            // library has no on-disk path (e.g. `linux-vdso.so.1`). Peel off the
            // address and treat an empty path as unresolved.
            let path_part = rest
                .split_once('(')
                .map(|(p, _)| p.trim())
                .unwrap_or_else(|| rest.trim());
            let resolved = if path_part.is_empty() {
                None
            } else {
                Some(PathBuf::from(path_part))
            };
            deps.push(Dep { name, resolved });
        } else {
            let name = line.split_whitespace().next().unwrap_or(line).to_string();
            deps.push(Dep {
                name,
                resolved: None,
            });
        }
    }
    deps
}

#[cfg(target_os = "macos")]
fn collect_otool(binary: &Path) -> Vec<Dep> {
    let output = match std::process::Command::new("otool")
        .args(["-L", binary.to_str().unwrap_or("")])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    let mut deps = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines().skip(1) {
        let line = line.trim();
        if let Some((path, _)) = line.split_once('(') {
            let path = path.trim();
            if let Some(name) = path.rsplit('/').next() {
                deps.push(Dep {
                    name: name.to_string(),
                    resolved: if path.starts_with('@') || std::path::Path::new(path).is_absolute() {
                        Some(PathBuf::from(path))
                    } else {
                        None
                    },
                });
            }
        }
    }
    deps
}

#[cfg(target_os = "windows")]
fn collect_pe_imports(binary: &Path) -> Vec<Dep> {
    // Best-effort: a full PE parser is out of scope; emit the DLLs the binary
    // imports by scanning its bytes for the ASCII `foo.dll` tails of the import
    // directory. This catches the common case (bundled Chromium DLLs) without a
    // PE dependency.
    let bytes = match std::fs::read(binary) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    let mut seen = std::collections::BTreeSet::new();
    let mut deps = Vec::new();
    let lower = |b: u8| b.to_ascii_lowercase();
    for window in bytes.windows(5) {
        if lower(window[1]) == b'.'
            && lower(window[2]) == b'd'
            && lower(window[3]) == b'l'
            && lower(window[4]) == b'l'
            && window[0].is_ascii_alphanumeric()
        {
            // walk back to the start of the name token
            // SAFETY: `window` is a 5-byte subslice of `bytes` from
            // `bytes.windows(5)`. `.add(5)` points one-past-the-window,
            // which lies within `bytes[..=bytes.len()]` (at most
            // one-past-the-end of the parent allocation). The pointer
            // difference yields a valid byte offset into `bytes`.
            let end = unsafe { window.as_ptr().add(5) as usize - bytes.as_ptr() as usize };
            let mut start = end - 5;
            while start > 0
                && (bytes[start - 1].is_ascii_alphanumeric()
                    || bytes[start - 1] == b'_'
                    || bytes[start - 1] == b'-'
                    || bytes[start - 1] == b'.')
            {
                start -= 1;
            }
            if let Ok(name) = std::str::from_utf8(&bytes[start..end]) {
                let name = name.to_ascii_lowercase();
                if seen.insert(name.clone()) {
                    deps.push(Dep {
                        name,
                        resolved: None,
                    });
                }
            }
        }
    }
    deps
}

/// A ready-to-print summary of everything a packager should ship alongside a
/// product that embeds shirabe.
#[derive(Debug, Default)]
pub struct BundleReport {
    /// Explicitly declared libs (from [`BundleSpec`]).
    pub declared: Vec<NativeLib>,
    /// Runtime dependencies discovered by scanning the backend binary.
    pub discovered: Vec<Dep>,
}

impl BundleReport {
    /// Build a report for `backend_exe`: merge the user's [`BundleSpec`] with
    /// the runtime deps discovered from the binary.
    pub fn build(backend_exe: &Path) -> Self {
        BundleReport {
            declared: BundleSpec::from_env().libs,
            discovered: collect_runtime_deps(backend_exe),
        }
    }
}

/// Render [`BundleReport`] as human-readable guidance for a release script.
pub fn render(report: &BundleReport) -> String {
    let mut out = String::new();
    out.push_str("# shirabe native-library bundle report\n\n");

    if !report.declared.is_empty() {
        out.push_str("## Declared libraries (ship these next to your product)\n");
        for lib in &report.declared {
            let exists = lib.path.exists();
            let flag = match (exists, lib.optional) {
                (true, _) => "ok",
                (false, true) => "missing (optional, ignored)",
                (false, false) => "MISSING",
            };
            let os = lib.target_os.clone().unwrap_or_else(|| "all".to_string());
            out.push_str(&format!("- {} [{os}] {flag}\n", lib.path.display()));
        }
        out.push('\n');
    }

    if !report.discovered.is_empty() {
        out.push_str("## Backend runtime dependencies (copy resolved paths)\n");
        for dep in &report.discovered {
            let where_ = dep
                .resolved
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "(unresolved — install on the target)".to_string());
            out.push_str(&format!("- {}  → {}\n", dep.name, where_));
        }
    }

    if report.declared.is_empty() && report.discovered.is_empty() {
        out.push_str("(nothing to bundle)\n");
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_parses_lib_tables() {
        let toml = r#"
[[lib]]
path = "libfoo.so"
optional = true
target_os = "linux"

[[lib]]
path = "foo.dll"
"#;
        let libs = parse_manifest(toml);
        assert_eq!(libs.len(), 2);
        assert_eq!(libs[0].path, PathBuf::from("libfoo.so"));
        assert!(libs[0].optional);
        assert_eq!(libs[0].target_os.as_deref(), Some("linux"));
        assert_eq!(libs[1].path, PathBuf::from("foo.dll"));
        assert!(!libs[1].optional);
    }

    #[test]
    fn render_handles_empty_report() {
        let report = BundleReport::default();
        let text = render(&report);
        assert!(text.contains("nothing to bundle"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn ldd_parser_handles_arrow_form() {
        let stdout = "\tlibnss3.so => /usr/lib/x86_64-linux-gnu/libnss3.so (0x00007f123)\n\tlinux-vdso.so.1 =>  (0x000)\n";
        let deps = parse_ldd(stdout);
        assert_eq!(deps.len(), 2);
        assert_eq!(deps[0].name, "libnss3.so");
        assert_eq!(
            deps[0].resolved.as_ref().unwrap().to_str().unwrap(),
            "/usr/lib/x86_64-linux-gnu/libnss3.so"
        );
        assert!(deps[1].resolved.is_none());
    }
}
