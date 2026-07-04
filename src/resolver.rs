//! Chrome executable resolver — zero-config like ort.
//!
//! Resolution order:
//! 1. `$CHROME_PATH` — explicit override
//! 2. Build-time baked path (`SHIRABE_BROWSER_PATH`, set by build.rs auto-fetch)
//! 3. System Chrome on PATH
//! 4. Runtime fetch — download Chrome for Testing into cache
//! 5. Error

pub use crate::browser_fetch::resolve;

/// Synchronous wrapper for use from async contexts.
pub fn resolve_executable() -> Result<String, String> {
    crate::browser_fetch::resolve()
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}
