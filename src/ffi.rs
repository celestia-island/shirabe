//! Foreign-engine FFI — the Firefox / Servo contract.
//!
//! ## The idea
//!
//! The Chromium family (Chrome / Chromium / Edge) is driven in-process through
//! our own CDP engine. Firefox and Servo are a different story: their cores are
//! enormous, so we let the **browser vendors** (or anyone building those cores)
//! compile a thin adapter against a tiny C ABI and ship it as a dynamic library
//! — exactly the model [ort](https://crates.io/crates/ort) uses for ONNX
//! Runtime. shirabe's job here is the "thin C-binding wrapper": a [`Engine`]
//! trait, the C ABI a vendor lib must export, and a [`CdylibEngine`] that
//! `dlopen`s it and routes calls.
//!
//! Vendor libs are expected as:
//!
//! - `libshirabe_engine_firefox.{so,dylib,dll}`
//! - `libshirabe_engine_servo.{so,dylib,dll}`
//!
//! …located in the shared cache or next to the binary. A release workflow
//! publishes prebuilt copies of those libs to GitHub Releases; the resolver
//! downloads them on first use just like Chrome for Testing.
//!
//! ## The C ABI (what a vendor lib exports)
//!
//! ```c
//! // Opaque handle to a browser session.
//! typedef struct shirabe_engine shirabe_engine;
//!
//! // Lifecycle. `options_json` is a JSON string of engine-specific options
//! // (user-data dir, proxy, viewport, …). Returns NULL on failure.
//! shirabe_engine *shirabe_engine_new(const char *options_json);
//! void            shirabe_engine_destroy(shirabe_engine *eng);
//!
//! // Navigate the active page to `url`. 0 on success, non-zero errno-style.
//! int   shirabe_engine_navigate(shirabe_engine *eng, const char *url);
//!
//! // Evaluate `js` and return the result as a JSON string owned by the engine
//! // (freed by the caller via shirabe_engine_free_string). NULL on failure.
//! char *shirabe_engine_evaluate(shirabe_engine *eng, const char *js);
//! void  shirabe_engine_free_string(shirabe_engine *eng, char *s);
//!
//! // Capture a PNG. Writes a heap buffer + length; caller frees with
//! // shirabe_engine_free_pixels. 0 on success.
//! int   shirabe_engine_screenshot(shirabe_engine *eng,
//!                                 unsigned char **out, size_t *out_len);
//! void  shirabe_engine_free_pixels(shirabe_engine *eng,
//!                                  unsigned char *buf, size_t len);
//!
//! // Engine identity ("firefox" / "servo" / …), static NUL-terminated.
//! const char *shirabe_engine_id(void);
//! ```
//!
//! This is small enough that a vendor adapter is a few hundred lines, yet
//! covers the automation surface shirabe exposes over HTTP.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::path::PathBuf;

use anyhow::{Result, anyhow};

use crate::backend::Backend;

/// A driveable browser engine, independent of the wire protocol.
pub trait Engine {
    fn id(&self) -> &str;
    fn navigate(&mut self, url: &str) -> Result<()>;
    fn evaluate(&mut self, js: &str) -> Result<String>;
    fn screenshot(&mut self) -> Result<Vec<u8>>;
}

/// C-ABI symbols a vendor engine library must export. Matches the header in
/// the module docs.
#[repr(C)]
struct EngineVtable {
    id: unsafe extern "C" fn() -> *const c_char,
    new: unsafe extern "C" fn(*const c_char) -> *mut std::ffi::c_void,
    destroy: unsafe extern "C" fn(*mut std::ffi::c_void),
    navigate: unsafe extern "C" fn(*mut std::ffi::c_void, *const c_char) -> c_int,
    evaluate: unsafe extern "C" fn(*mut std::ffi::c_void, *const c_char) -> *mut c_char,
    free_string: unsafe extern "C" fn(*mut std::ffi::c_void, *mut c_char),
    screenshot: unsafe extern "C" fn(*mut std::ffi::c_void, *mut *mut u8, *mut usize) -> c_int,
    free_pixels: unsafe extern "C" fn(*mut std::ffi::c_void, *mut u8, usize),
}

/// A vendor engine loaded via `dlopen`. Holds the library + one session handle.
pub struct CdylibEngine {
    #[allow(dead_code)]
    lib: libloading::Library,
    vt: EngineVtable,
    handle: *mut std::ffi::c_void,
    id: String,
}

unsafe impl Send for CdylibEngine {}

impl CdylibEngine {
    /// Open the vendor library for `backend` and start a session with
    /// `options_json` (engine-specific: user-data dir, proxy, viewport, …).
    ///
    /// Resolution order for the library file:
    /// 1. `SHIRABE_ENGINE_PATH` — explicit path to the vendor lib.
    /// 2. Next to the current executable (`<exe>/libshirabe_engine_<id>…`).
    /// 3. The shared cache (`<cache>/shirabe/engines/<id>/…`), where the
    ///    release-fetch step places downloaded copies.
    pub fn open(backend: Backend, options_json: &str) -> Result<Self> {
        let id = backend.engine_id().ok_or_else(|| {
            anyhow!(
                "backend {} is CDP-driven in-process; it has no foreign engine lib",
                backend.label()
            )
        })?;
        let path = locate_engine_lib(id)?;
        let lib = unsafe { libloading::Library::new(&path) }
            .map_err(|e| anyhow!("dlopen {} failed: {e}", path.display()))?;

        // Resolve the whole vtable up front so later calls can't fail halfway.
        // `Library::get` is safe in libloading 0.8; it returns a `Symbol`, of
        // which we copy out the function pointer (the call itself is unsafe).
        macro_rules! sym {
            ($name:literal) => {{
                let label = std::str::from_utf8($name).unwrap_or("<symbol>");
                // SAFETY: loading a symbol by name is safe in principle; the
                // unsafety is the later call through the fn pointer.
                *unsafe { lib.get($name) }.map_err(|e| anyhow!("missing {label}: {e}"))?
            }};
        }
        let vt = EngineVtable {
            id: sym!(b"shirabe_engine_id\0"),
            new: sym!(b"shirabe_engine_new\0"),
            destroy: sym!(b"shirabe_engine_destroy\0"),
            navigate: sym!(b"shirabe_engine_navigate\0"),
            evaluate: sym!(b"shirabe_engine_evaluate\0"),
            free_string: sym!(b"shirabe_engine_free_string\0"),
            screenshot: sym!(b"shirabe_engine_screenshot\0"),
            free_pixels: sym!(b"shirabe_engine_free_pixels\0"),
        };

        let opts_c = CString::new(options_json)
            .map_err(|e| anyhow!("embedded NUL in engine options: {e}"))?;
        let handle = unsafe { (vt.new)(opts_c.as_ptr()) };
        if handle.is_null() {
            return Err(anyhow!("vendor engine {} refused to start", id));
        }

        let id_str = unsafe { CStr::from_ptr((vt.id)()) }
            .to_string_lossy()
            .into_owned();

        Ok(CdylibEngine {
            lib,
            vt,
            handle,
            id: id_str,
        })
    }
}

impl Engine for CdylibEngine {
    fn id(&self) -> &str {
        &self.id
    }

    fn navigate(&mut self, url: &str) -> Result<()> {
        let c = CString::new(url).map_err(|e| anyhow!("embedded NUL in navigate URL: {e}"))?;
        let rc = unsafe { (self.vt.navigate)(self.handle, c.as_ptr()) };
        if rc == 0 {
            Ok(())
        } else {
            Err(anyhow!("vendor navigate failed (code {rc})"))
        }
    }

    fn evaluate(&mut self, js: &str) -> Result<String> {
        let c = CString::new(js).map_err(|e| anyhow!("embedded NUL in JS expression: {e}"))?;
        let ptr = unsafe { (self.vt.evaluate)(self.handle, c.as_ptr()) };
        if ptr.is_null() {
            return Err(anyhow!("vendor evaluate returned null"));
        }
        let out = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        unsafe { (self.vt.free_string)(self.handle, ptr) };
        Ok(out)
    }

    fn screenshot(&mut self) -> Result<Vec<u8>> {
        let mut buf: *mut u8 = std::ptr::null_mut();
        let mut len: usize = 0;
        let rc = unsafe { (self.vt.screenshot)(self.handle, &mut buf, &mut len) };
        if rc != 0 || buf.is_null() {
            return Err(anyhow!("vendor screenshot failed (code {rc})"));
        }
        let out = unsafe { std::slice::from_raw_parts(buf, len) }.to_vec();
        unsafe { (self.vt.free_pixels)(self.handle, buf, len) };
        Ok(out)
    }
}

impl Drop for CdylibEngine {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { (self.vt.destroy)(self.handle) };
            self.handle = std::ptr::null_mut();
        }
    }
}

/// Resolve the vendor library path for engine `id` (e.g. `"firefox"`).
pub fn locate_engine_lib(id: &str) -> Result<PathBuf> {
    let file = lib_name(id);

    if let Ok(p) = std::env::var("SHIRABE_ENGINE_PATH") {
        let path = PathBuf::from(&p);
        if path.exists() {
            return Ok(path);
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let cand = dir.join(&file);
            if cand.exists() {
                return Ok(cand);
            }
        }
    }

    let cached = cache_engine_path(id).join(&file);
    if cached.exists() {
        return Ok(cached);
    }

    Err(anyhow!(
        "no vendor engine lib for `{id}` found. Set SHIRABE_ENGINE_PATH, drop \
         `{file}` next to the executable, or let shirabe fetch it from a release \
         into {}",
        cache_engine_path(id).display()
    ))
}

/// `<cache>/shirabe/engines/<id>` — where release-fetched vendor libs land.
pub fn cache_engine_path(id: &str) -> PathBuf {
    cache_dir()
        .unwrap_or_else(|| std::env::temp_dir().join("shirabe-cache"))
        .join("shirabe")
        .join("engines")
        .join(id)
}

fn cache_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Caches"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
    {
        None
    }
}

/// Platform library filename for engine `id`.
fn lib_name(id: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("shirabe_engine_{id}.dll")
    } else if cfg!(target_os = "macos") {
        format!("libshirabe_engine_{id}.dylib")
    } else {
        format!("libshirabe_engine_{id}.so")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lib_name_is_platform_appropriate() {
        let n = lib_name("firefox");
        if cfg!(target_os = "windows") {
            assert_eq!(n, "shirabe_engine_firefox.dll");
        } else if cfg!(target_os = "macos") {
            assert_eq!(n, "libshirabe_engine_firefox.dylib");
        } else {
            assert_eq!(n, "libshirabe_engine_firefox.so");
        }
    }

    #[test]
    fn missing_engine_lib_is_a_clean_error() {
        // No firefox lib shipped in the test env — must Err, not panic.
        // SAFETY: serial-ish unit test; no concurrent reader of this var.
        unsafe { std::env::remove_var("SHIRABE_ENGINE_PATH") };
        assert!(locate_engine_lib("definitely_no_such_engine").is_err());
    }
}
