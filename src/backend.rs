//! Browser backend selection.
//!
//! Shirabe can drive any browser that speaks the Chrome DevTools Protocol.
//! Today that covers the whole Chromium family — Google Chrome, Chromium and
//! Microsoft Edge — all driven by the same CDP engine in [`crate::engine`].
//!
//! A backend is chosen at runtime, following the same ort-style "find it, or
//! fetch it" philosophy as the Chrome-for-Testing downloader in
//! [`crate::browser_fetch`]:
//!
//! 1. **Backend-specific env override** — `CHROME_PATH`, `CHROMIUM_PATH` or
//!    `EDGE_PATH` pin a backend to an explicit executable.
//! 2. **Build-time baked path** — `SHIRABE_BROWSER_PATH`, emitted by `build.rs`
//!    when the `auto-fetch` feature downloads Chrome for Testing during the
//!    build.
//! 3. **System binary on `$PATH`** (and a handful of well-known install
//!    locations), scanned in backend order.
//! 4. **Runtime fetch** — download the pinned Chrome for Testing build into the
//!    shared cache (the `runtime-fetch` feature).
//!
//! Select a backend explicitly with `SHIRABE_BACKEND=chrome|chromium|edge|auto`
//! (default `auto`). Whatever is chosen, [`resolve`] returns the executable
//! path; the CDP engine then drives it uniformly.

use std::path::{Path, PathBuf};

use crate::browser_fetch;

/// A CDP-speaking browser backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// Google Chrome (stable) — system install or Chrome for Testing fetch.
    Chrome,
    /// Chromium — the open-source build.
    Chromium,
    /// Microsoft Edge (Chromium-based).
    Edge,
    /// Mozilla Firefox — driven through the FFI engine contract
    /// (`libshirabe_engine_firefox`), not CDP. Requires the `foreign-engine`
    /// feature + a published vendor lib.
    Firefox,
    /// Servo — driven through the FFI engine contract
    /// (`libshirabe_engine_servo`). Requires the `foreign-engine` feature.
    Servo,
    /// Let shirabe pick the first backend that resolves. This is the default.
    Auto,
}

impl Backend {
    /// The backend selected via `SHIRABE_BACKEND`, or `Auto`.
    pub fn from_env() -> Self {
        match std::env::var("SHIRABE_BACKEND")
            .ok()
            .map(|s| s.trim().to_ascii_lowercase())
            .as_deref()
        {
            Some("chrome") => Backend::Chrome,
            Some("chromium") => Backend::Chromium,
            Some("edge") => Backend::Edge,
            Some("firefox") => Backend::Firefox,
            Some("servo") => Backend::Servo,
            _ => Backend::Auto,
        }
    }

    /// Iterate the concrete backends to try, in order. `Auto` expands to the
    /// CDP preference list (Chrome first). Firefox/Servo are never tried
    /// implicitly — they are opt-in via `SHIRABE_BACKEND` because they need a
    /// separately-published vendor engine lib.
    pub fn order(self) -> &'static [Backend] {
        // Each arm is a `&'static` slice, so there is no temporary to borrow.
        const AUTO: &[Backend] = &[Backend::Chrome, Backend::Chromium, Backend::Edge];
        const CHROME: &[Backend] = &[Backend::Chrome];
        const CHROMIUM: &[Backend] = &[Backend::Chromium];
        const EDGE: &[Backend] = &[Backend::Edge];
        const FIREFOX: &[Backend] = &[Backend::Firefox];
        const SERVO: &[Backend] = &[Backend::Servo];
        match self {
            Backend::Auto => AUTO,
            Backend::Chrome => CHROME,
            Backend::Chromium => CHROMIUM,
            Backend::Edge => EDGE,
            Backend::Firefox => FIREFOX,
            Backend::Servo => SERVO,
        }
    }

    /// Human-readable label, used in logs and the `/info` endpoint.
    pub fn label(self) -> &'static str {
        match self {
            Backend::Chrome => "chrome",
            Backend::Chromium => "chromium",
            Backend::Edge => "edge",
            Backend::Firefox => "firefox",
            Backend::Servo => "servo",
            Backend::Auto => "auto",
        }
    }

    /// For foreign (non-CDP) backends, the vendor engine id used to locate the
    /// dynamic library. `None` for the CDP family, which is driven in-process.
    pub fn engine_id(self) -> Option<&'static str> {
        match self {
            Backend::Firefox => Some("firefox"),
            Backend::Servo => Some("servo"),
            _ => None,
        }
    }

    /// `true` for the CDP family (driven by our own engine, no vendor lib).
    pub fn is_cdp(self) -> bool {
        matches!(self, Backend::Chrome | Backend::Chromium | Backend::Edge)
    }

    /// `$PATH` / well-known-location candidates for this backend on the host.
    /// Only meaningful for the CDP family; foreign backends resolve a vendor
    /// library via [`crate::ffi`], not an executable.
    fn candidates(self) -> Vec<PathBuf> {
        if !self.is_cdp() {
            return Vec::new();
        }
        system_candidates(self)
    }

    /// Env vars that explicitly override this backend's executable, in order.
    fn env_overrides(self) -> &'static [&'static str] {
        match self {
            Backend::Chrome => &["CHROME_PATH"],
            Backend::Chromium => &["CHROMIUM_PATH"],
            Backend::Edge => &["EDGE_PATH"],
            // Foreign backends are pinned by SHIRABE_ENGINE_PATH at the FFI
            // layer, not by a per-backend executable env here.
            Backend::Firefox | Backend::Servo | Backend::Auto => &[],
        }
    }
}

/// Resolve the selected backend and an executable for it, following the
/// ort-style order documented at the top of this module.
///
/// Foreign backends (Firefox / Servo) are **not** resolved here — they have no
/// executable to launch, only a vendor engine library. Callers that select one
/// should drive it via [`crate::ffi::CdylibEngine::open`] instead. This function
/// returns a dedicated error for them so the CDP engine never tries to spawn
/// `firefox` as if it spoke CDP.
///
/// **Blocking:** the runtime-fetch fallback may perform a multi-second HTTP
/// download + zip extraction. Do not call it on an async worker thread; wrap it
/// in `tokio::task::spawn_blocking` (the engine does this).
pub fn resolve() -> anyhow::Result<(Backend, PathBuf)> {
    let selected = Backend::from_env();

    // Foreign backend: defer to the FFI layer — there is no executable to find.
    if let Some(id) = selected.engine_id() {
        return Err(anyhow::anyhow!(
            "backend `{}` is driven through the FFI engine contract, not as a \
             spawned executable. Open it via shirabe::ffi::CdylibEngine::open; \
             the vendor lib `libshirabe_engine_{id}` is resolved separately.",
            selected.label()
        ));
    }

    for backend in selected.order() {
        // 1. Backend-specific explicit override.
        for var in backend.env_overrides() {
            if let Ok(p) = std::env::var(var) {
                if !p.is_empty() {
                    let path = PathBuf::from(&p);
                    if path.exists() {
                        return Ok((*backend, path));
                    }
                    anyhow::bail!(
                        "{var} is set to {p:?} but it does not exist",
                        var = var,
                        p = p
                    );
                }
            }
        }

        // 2. Build-time baked Chrome-for-Testing path (set by build.rs under
        // `auto-fetch`). Tried before any stray system Chrome so the pinned
        // build wins — matching the documented resolution order. Only relevant
        // for the Chrome backend (the baked path is always Chrome for Testing).
        if *backend == Backend::Chrome {
            if let Some(p) = option_env!("SHIRABE_BROWSER_PATH") {
                if !p.is_empty() && Path::new(p).exists() {
                    return Ok((Backend::Chrome, PathBuf::from(p)));
                }
            }
        }

        // 3. System binary on PATH / well-known locations.
        if let Some(p) = backend.candidates().into_iter().next() {
            return Ok((*backend, p));
        }
    }

    // 4. Runtime fetch — Chrome for Testing, so it resolves to the Chrome
    // backend regardless of selection.
    match browser_fetch::resolve() {
        Ok(path) => Ok((Backend::Chrome, path)),
        Err(e) => Err(e),
    }
}

/// Stringly-typed wrapper for callers (e.g. the engine) that only need the path.
pub fn resolve_executable() -> Result<String, String> {
    resolve()
        .map(|(_, p)| p.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}

/// Probe `$PATH` (and a handful of well-known install locations) for the first
/// executable belonging to `backend`. Mirrors the lookup `browser_fetch` does
/// for Chrome, generalised to the whole Chromium family.
fn system_candidates(backend: Backend) -> Vec<PathBuf> {
    let names: &[&str] = match backend {
        Backend::Chrome => &[
            "google-chrome",
            "google-chrome-stable",
            "chrome",
            "chromium-browser",
            "chromium",
        ],
        Backend::Chromium => &["chromium", "chromium-browser"],
        Backend::Edge => &["microsoft-edge", "microsoft-edge-stable"],
        // Foreign backends reach this fn only defensively; return nothing.
        Backend::Firefox | Backend::Servo => &[],
        Backend::Auto => &[
            "google-chrome",
            "google-chrome-stable",
            "chromium-browser",
            "chromium",
            "microsoft-edge",
        ],
    };

    let mut hits = Vec::new();

    if let Some(path_var) = std::env::var_os("PATH") {
        let try_names: Vec<String> = if cfg!(windows) {
            let mut v: Vec<String> = names.iter().map(|s| format!("{s}.exe")).collect();
            v.extend(names.iter().map(|s| s.to_string()));
            v
        } else {
            names.iter().map(|s| s.to_string()).collect()
        };
        for dir in std::env::split_paths(&path_var) {
            for name in &try_names {
                let candidate = dir.join(name);
                if is_executable_file(&candidate) {
                    hits.push(candidate);
                }
            }
        }
    }

    for p in well_known_locations(backend) {
        let candidate = PathBuf::from(p);
        if is_executable_file(&candidate) {
            hits.push(candidate);
        }
    }

    hits
}

fn well_known_locations(backend: Backend) -> &'static [&'static str] {
    match (cfg!(target_os = "macos"), cfg!(target_os = "windows")) {
        (true, _) => match backend {
            Backend::Chromium => &["/Applications/Chromium.app/Contents/MacOS/Chromium"],
            Backend::Edge => &["/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"],
            // Auto + Chrome fall back to Chrome's well-known macOS path.
            _ => &["/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"],
        },
        (false, true) => match backend {
            Backend::Chromium => &[r"C:\Program Files\Chromium\Application\chrome.exe"],
            Backend::Edge => &[r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"],
            _ => &[
                r"C:\Program Files\Google\Chrome\Application\chrome.exe",
                r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
            ],
        },
        _ => match backend {
            Backend::Chromium => &[
                "/usr/bin/chromium",
                "/usr/bin/chromium-browser",
                "/snap/bin/chromium",
            ],
            Backend::Edge => &["/usr/bin/microsoft-edge", "/usr/bin/microsoft-edge-stable"],
            _ => &[
                "/usr/bin/google-chrome",
                "/usr/bin/google-chrome-stable",
                "/snap/bin/chromium",
            ],
        },
    }
}

fn is_executable_file(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.is_file())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_expands_to_preference_order() {
        assert_eq!(
            Backend::Auto.order(),
            &[Backend::Chrome, Backend::Chromium, Backend::Edge]
        );
        assert_eq!(Backend::Edge.order(), &[Backend::Edge]);
    }

    #[test]
    fn env_overrides_match_backend() {
        assert_eq!(Backend::Chrome.env_overrides(), &["CHROME_PATH"]);
        assert_eq!(Backend::Chromium.env_overrides(), &["CHROMIUM_PATH"]);
        assert_eq!(Backend::Edge.env_overrides(), &["EDGE_PATH"]);
        assert!(Backend::Auto.env_overrides().is_empty());
    }

    #[test]
    #[serial_test::serial]
    fn from_env_parses_known_values() {
        let restore = std::env::var_os("SHIRABE_BACKEND");
        // SAFETY: tests are run single-threaded for env mutation under the
        // `serial` lock; no other thread reads `SHIRABE_BACKEND` concurrently.
        for (raw, expected) in [
            ("chrome", Backend::Chrome),
            ("  Chromium ", Backend::Chromium),
            ("EDGE", Backend::Edge),
            ("firefox", Backend::Firefox),
            ("SERVO", Backend::Servo),
            ("unknown", Backend::Auto),
        ] {
            unsafe { std::env::set_var("SHIRABE_BACKEND", raw) };
            assert_eq!(Backend::from_env(), expected, "raw = {raw:?}");
        }
        unsafe {
            match &restore {
                Some(v) => std::env::set_var("SHIRABE_BACKEND", v),
                None => std::env::remove_var("SHIRABE_BACKEND"),
            }
        }
    }

    #[test]
    #[serial_test::serial]
    fn foreign_backends_defer_to_ffi_layer() {
        let restore = std::env::var_os("SHIRABE_BACKEND");
        unsafe { std::env::set_var("SHIRABE_BACKEND", "firefox") };
        // A foreign backend must NOT resolve a spawned executable — the error
        // points the caller at the FFI engine contract instead.
        let err = resolve().unwrap_err().to_string();
        assert!(err.contains("FFI"), "unexpected error: {err}");
        assert_eq!(Backend::Firefox.engine_id(), Some("firefox"));
        assert_eq!(Backend::Servo.engine_id(), Some("servo"));
        assert!(Backend::Chrome.engine_id().is_none());
        assert!(!Backend::Firefox.is_cdp());
        unsafe {
            match restore {
                Some(v) => std::env::set_var("SHIRABE_BACKEND", v),
                None => std::env::remove_var("SHIRABE_BACKEND"),
            }
        }
    }
}
