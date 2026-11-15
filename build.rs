//! ort-style build-time Chromium resolver. When the `auto-fetch` feature is on,
//! this downloads the pinned Chrome for Testing into a shared cache (once, then
//! cached across builds) and bakes the path into the library via
//! `cargo:rustc-env=SHIRABE_BROWSER_PATH=...`.
//!
//! Every downloaded archive is checked against a SHA-256 pinned in
//! [`CHROME_SHA256`] — the checksum is a build *gate*, not an opt-in. The
//! `SHIRABE_CHROME_SHA256` env var only *overrides* the pinned digest, and an
//! unverifiable download aborts the build instead of silently falling back.
//!
//! The gate also covers the cache: an install writes
//! `<cache>/<flavor>/<ver>/<id>/.shirabe-verified` holding the digest its archive
//! was checked against, and a later build trusts a cache tree only while that
//! marker records the digest it would require *now*. A tree without a matching
//! marker (pre-marker cache, CI-restored cache, cached Docker layer) is never
//! trusted on the strength of "it is already on disk": the tree is not re-hashed
//! but replaced by a fresh verified install.
//!
//! KEEP IN SYNC with `src/lib.rs` (version, URL scheme, cache layout).

use std::path::{Path, PathBuf};

const CHROME_VERSION: &str = "150.0.7871.46";
const DEFAULT_MIRROR: &str = "https://storage.googleapis.com/chrome-for-testing-public";

/// Install-time marker inside a cache entry: it holds the (lowercase hex) digest
/// the extracted tree was verified against, and is the only thing that makes a
/// later build trust the tree without re-downloading it.
const MARKER_FILE: &str = ".shirabe-verified";

/// SHA-256 of the pinned Chrome-for-Testing archives, keyed by
/// `(version, flavor, download id)`.
///
/// This table is the default integrity gate, so that an outdated, broken, or
/// hostile mirror can never hand the build a different browser binary. There is
/// no "trust the download" mode: a missing entry is a build failure, and
/// `SHIRABE_CHROME_SHA256` only *overrides* the digest (local mirror, or a
/// version bump done ahead of refreshing this table).
///
/// ⚠️ Bumping `CHROME_VERSION` — or changing which flavors / download ids are
/// supported — REQUIRES refreshing this table in the same commit: fetch every
/// archive listed below from `DEFAULT_MIRROR`, `sha256sum` it, and paste the
/// digests here. A stale entry makes the build fail loudly on the first
/// uncached download; that is deliberate, never ship an unverified browser.
const CHROME_SHA256: &[(&str, &str, &str, &str)] = &[
    (
        CHROME_VERSION,
        "shell",
        "linux64",
        "0395e5db8d1631d5ed879d4f05fec423425537018fa1d73a58b1ade13288f447",
    ),
    (
        CHROME_VERSION,
        "shell",
        "mac-arm64",
        "38d663dcf205c8fc210cde9238f4b7f55c5f3757ec89f325ce0ec53951a842e7",
    ),
    (
        CHROME_VERSION,
        "shell",
        "mac-x64",
        "50b42d8313d44dc38c696fc2851e8c7dd2fd95b3dd69e05134abdab143d813c5",
    ),
    (
        CHROME_VERSION,
        "shell",
        "win64",
        "b97b40bbad365db24c69c00d3150c9dfd10012d16eb5da82e1fb73984a7b7f88",
    ),
    (
        CHROME_VERSION,
        "full",
        "linux64",
        "ad115a7498a17f53f6ed0914458326c6516addc756224db14c32184a9b1ab078",
    ),
    (
        CHROME_VERSION,
        "full",
        "mac-arm64",
        "9b6c6fbbda6f2948c74425828bd18f0e59542fd2beb056697478c38a956d83cb",
    ),
    (
        CHROME_VERSION,
        "full",
        "mac-x64",
        "dc6280df5fa5cb1da96f7026a20d7622df921fa5ba03c07e95af4a8dde17b131",
    ),
    (
        CHROME_VERSION,
        "full",
        "win64",
        "2245f764e024897195bce1cf6ff16d60544351e4d1b68edb6956ea875ab8b929",
    ),
];

/// Integrity failure — the bytes we got are not provably the bytes we pinned
/// (including "no digest is pinned for this combination"). Unlike a transport
/// error, this must abort the build: falling back would ship an unverified
/// browser. `main` downcasts to this to tell the two apart.
#[derive(Debug)]
struct ChecksumError(String);

impl std::fmt::Display for ChecksumError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ChecksumError {}

/// Pinned digest for `(version, flavor, download id)`; `None` = not pinned.
fn pinned_sha256(ver: &str, flavor: &str, id: &str) -> Option<&'static str> {
    let ver = ver.trim();
    CHROME_SHA256
        .iter()
        .find(|(v, f, i, _)| *v == ver && *f == flavor && *i == id)
        .map(|(_, _, _, digest)| *digest)
}

/// The digest the downloaded body must match: `SHIRABE_CHROME_SHA256` (hex)
/// when set — an explicit override — otherwise the pinned table. Refusing to
/// invent a default is the point: no digest ⇒ no download.
fn expected_sha256(ver: &str, flavor: &str, id: &str) -> anyhow::Result<String> {
    let overridden = std::env::var("SHIRABE_CHROME_SHA256")
        .ok()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());
    if let Some(hex) = overridden {
        return Ok(hex);
    }
    match pinned_sha256(ver, flavor, id) {
        Some(hex) => Ok(hex.to_string()),
        None => Err(anyhow::Error::new(ChecksumError(format!(
            "no pinned sha256 for chrome {ver} ({flavor}/{id}): add the digest to \
             CHROME_SHA256 in build.rs, or set SHIRABE_CHROME_SHA256"
        )))),
    }
}

fn main() {
    println!("cargo:rerun-if-env-changed=SHIRABE_CHROME_VERSION");
    println!("cargo:rerun-if-env-changed=SHIRABE_CHROME_MIRROR");
    println!("cargo:rerun-if-env-changed=SHIRABE_CHROME_SHA256");
    println!("cargo:rerun-if-env-changed=SHIRABE_SKIP_BROWSER_FETCH");
    println!("cargo:rerun-if-env-changed=SHIRABE_DOWNLOAD_PROXY");
    println!("cargo:rerun-if-env-changed=SHIRABE_DOWNLOAD_TIMEOUT_SECS");
    println!("cargo:rerun-if-env-changed=CHROME_PATH");
    // Cache-location vars (cache_dir() reads these); a change must re-bake.
    println!("cargo:rerun-if-env-changed=HOME");
    println!("cargo:rerun-if-env-changed=XDG_CACHE_HOME");
    println!("cargo:rerun-if-env-changed=LOCALAPPDATA");

    if std::env::var_os("CARGO_FEATURE_AUTO_FETCH").is_none() {
        return;
    }

    // Flavor: shell is the implicit default; `full` opts into full Chrome.
    let flavor = if std::env::var_os("CARGO_FEATURE_FULL").is_some() {
        "full"
    } else {
        "shell"
    };

    if std::env::var_os("SHIRABE_SKIP_BROWSER_FETCH").is_some() {
        eprintln!("[shirabe] SHIRABE_SKIP_BROWSER_FETCH set; skipping build-time fetch");
        return;
    }

    let ver =
        std::env::var("SHIRABE_CHROME_VERSION").unwrap_or_else(|_| CHROME_VERSION.to_string());
    let Some(id) = target_download_id() else {
        eprintln!("[shirabe] unsupported target; skipping build-time fetch");
        return;
    };

    let exec = installed_executable(flavor, &ver, id);
    // A cache tree is trusted only when its install-time marker records the
    // digest this build would hold a fresh download to. The extracted bytes are
    // never re-hashed: the marker is the entire argument for trusting the tree,
    // and anything short of an exact match means "unverified" — the tree is
    // replaced by a fresh install rather than used on the strength of existing
    // on disk (old caches, CI-restored caches and cached Docker layers are all
    // in that category).
    let verified_hit = if exec.exists() {
        match expected_sha256(&ver, flavor, id) {
            Ok(expected) => match marker_verdict(flavor, &ver, id, &expected) {
                MarkerVerdict::Verified => true,
                MarkerVerdict::Untrusted(why) => {
                    eprintln!(
                        "[shirabe] cache entry {} is unverified ({why}); \
                         reinstalling from the pinned mirror",
                        version_dir(flavor, &ver, id).display()
                    );
                    false
                }
            },
            Err(e) => {
                // Cannot state what a verified tree would have to be ⇒ cannot
                // vouch for this one. `download` re-raises the same error as a
                // fatal integrity failure.
                eprintln!(
                    "[shirabe] cache entry {} cannot be verified ({e}); \
                     reinstalling from the pinned mirror",
                    version_dir(flavor, &ver, id).display()
                );
                false
            }
        }
    } else {
        false
    };
    let final_path = if verified_hit {
        exec
    } else {
        match download(flavor, &ver, id) {
            Ok(p) => p,
            // Integrity is a hard gate: bytes we cannot verify must never be
            // silently replaced by "just use whatever browser is around".
            Err(e) if e.downcast_ref::<ChecksumError>().is_some() => {
                eprintln!("[shirabe] BUILD ABORTED: refusing an unverified Chrome download");
                eprintln!("[shirabe]   {e}");
                eprintln!(
                    "[shirabe] fix the mirror/network, pin the expected digest with \
                     SHIRABE_CHROME_SHA256, or set SHIRABE_SKIP_BROWSER_FETCH to build \
                     without a build-time browser"
                );
                std::process::exit(1);
            }
            Err(e) => {
                // Non-fatal (transport only): fall back to runtime / system
                // resolution. Integrity failures never reach this arm.
                eprintln!(
                    "[shirabe] build-time download failed ({e}); \
                     falling back to runtime resolution"
                );
                return;
            }
        }
    };

    // Propagate the resolved version unconditionally so lib.rs `version()`
    // (option_env!) and the build-time download stay in lock-step even if the
    // cache path itself isn't valid UTF-8.
    println!("cargo:rustc-env=SHIRABE_CHROME_VERSION={ver}");
    if let Some(s) = final_path.to_str() {
        println!("cargo:rustc-env=SHIRABE_BROWSER_PATH={s}");
    } else {
        eprintln!(
            "[shirabe] cache path is not valid UTF-8; \
             SHIRABE_BROWSER_PATH not emitted"
        );
    }
}

/// Chrome-for-Testing download id for the *target* triple (not the host).
fn target_download_id() -> Option<&'static str> {
    let os = std::env::var("CARGO_CFG_TARGET_OS").ok()?;
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").ok()?;
    match (os.as_str(), arch.as_str()) {
        ("linux", "x86_64") => Some("linux64"),
        ("macos", "aarch64") => Some("mac-arm64"),
        ("macos", "x86_64") => Some("mac-x64"),
        // Chrome for Testing only ships a win64 build; 32-bit Windows has no match.
        ("windows", "x86_64") => Some("win64"),
        _ => None,
    }
}

fn cache_root() -> PathBuf {
    cache_dir()
        .unwrap_or_else(|| std::env::temp_dir().join("tairitsu-cache"))
        .join("tairitsu")
        .join("browsers")
        .join("chromium")
}

/// `<cache>/.../chromium/<flavor>/<ver>/<id>` — the archive extracts *into* this.
fn version_dir(flavor: &str, ver: &str, id: &str) -> PathBuf {
    cache_root().join(flavor).join(ver).join(id)
}

/// Host cache directory (mirrors the `dirs` crate, inlined). The cache lives on
/// the build host even when cross-compiling.
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

fn installed_executable(flavor: &str, ver: &str, id: &str) -> PathBuf {
    let (top, exec): (String, String) = match flavor {
        "shell" => {
            let name = if id == "win64" {
                "chrome-headless-shell.exe"
            } else {
                "chrome-headless-shell"
            };
            (format!("chrome-headless-shell-{id}"), name.to_string())
        }
        _ => {
            let exec = match id {
                "linux64" => "chrome",
                "win64" => "chrome.exe",
                _ => "Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing",
            };
            (format!("chrome-{id}"), exec.to_string())
        }
    };
    cache_root()
        .join(flavor)
        .join(ver)
        .join(id)
        .join(top)
        .join(exec)
}

/// `<cache>/<flavor>/<ver>/<id>/.shirabe-verified` — written by the install that
/// populated this cache entry, read by every later build that finds it.
fn marker_path(flavor: &str, ver: &str, id: &str) -> PathBuf {
    version_dir(flavor, ver, id).join(MARKER_FILE)
}

/// What the install-time marker says about a tree that is already on disk.
#[derive(Debug, PartialEq, Eq)]
enum MarkerVerdict {
    /// The marker records exactly the digest a fresh install would be held to.
    Verified,
    /// Absent, unreadable, or recording some other digest — with the reason.
    Untrusted(String),
}

/// Compare marker contents (`None` = absent) against the `expected` digest.
///
/// Deliberately pure — no filesystem, no env — so the three states (absent /
/// recorded-something-else / match) are decision points that can be exercised
/// without a network, an archive, or a real cache.
fn classify_marker(marker: Option<&str>, expected: &str) -> MarkerVerdict {
    let Some(marker) = marker else {
        return MarkerVerdict::Untrusted(format!("no {MARKER_FILE} marker"));
    };
    let recorded = marker.trim();
    if recorded.is_empty() {
        return MarkerVerdict::Untrusted(format!("{MARKER_FILE} marker is empty"));
    }
    let expected = expected.trim();
    if !recorded.eq_ignore_ascii_case(expected) {
        return MarkerVerdict::Untrusted(format!(
            "{MARKER_FILE} marker records {recorded}, expected {expected}"
        ));
    }
    MarkerVerdict::Verified
}

/// Read the cache entry's marker and classify it. Any read failure other than
/// "absent" is also "untrusted": a marker we cannot read is a marker we cannot
/// use as evidence.
fn marker_verdict(flavor: &str, ver: &str, id: &str, expected: &str) -> MarkerVerdict {
    let path = marker_path(flavor, ver, id);
    match std::fs::read_to_string(&path) {
        Ok(contents) => classify_marker(Some(&contents), expected),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => classify_marker(None, expected),
        Err(e) => MarkerVerdict::Untrusted(format!("{} is unreadable ({e})", path.display())),
    }
}

fn archive_url(flavor: &str, ver: &str, id: &str) -> String {
    let raw = std::env::var("SHIRABE_CHROME_MIRROR").unwrap_or_else(|_| DEFAULT_MIRROR.to_string());
    let base = raw.trim_end_matches('/'); // avoid `//` if the mirror ends with `/`
    let stem = match flavor {
        "shell" => format!("chrome-headless-shell-{id}"),
        _ => format!("chrome-{id}"),
    };
    format!("{base}/{ver}/{id}/{stem}.zip")
}

/// Install the `ring` crypto provider as the process default. reqwest is built
/// with `rustls-no-provider`, so it has no TLS provider until this runs.
fn install_ring_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

fn download(flavor: &str, ver: &str, id: &str) -> anyhow::Result<PathBuf> {
    let url = archive_url(flavor, ver, id);
    // Resolve the expected digest before touching the network (and before the
    // temp-dir bookkeeping): no digest ⇒ no download.
    let expected = expected_sha256(ver, flavor, id)?;
    let target = installed_executable(flavor, ver, id);
    let dest = version_dir(flavor, ver, id);
    // Extract into a sibling temp dir, then swap into place, so a
    // partial/interrupted extraction never poisons the cache. The nonce makes
    // the temp path unique per call within a process.
    let parent = dest.parent().unwrap_or(Path::new("."));
    // Best-effort: reap stale temp dirs left by a previous crashed run (>1h old).
    sweep_stale_temps(parent, std::time::Duration::from_secs(3600));
    let nonce = TMP_NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = parent.join(format!(".{id}-{}-{nonce}.tmp", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);

    let result = download_inner(&url, &expected, &tmp, &dest);
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&tmp);
        return result.map(|()| target);
    }
    // Post-extract integrity: the expected binary must actually exist, else the
    // archive was truncated/malformed and we must not bake a dangling path.
    if !target.exists() {
        let _ = std::fs::remove_dir_all(&dest);
        anyhow::bail!(
            "extraction completed but the expected binary {} is missing",
            target.display()
        );
    }
    Ok(target)
}

fn download_inner(url: &str, expected: &str, tmp: &Path, dest: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(tmp)?;
    eprintln!("[shirabe] downloading {url} (once, then cached)");
    let bytes = fetch_with_retry(url, expected)?;

    extract_zip(&bytes, tmp)?;

    // Record provenance *inside* the temp tree, so the marker is swapped into
    // the cache together with the bytes it vouches for: an interrupted install
    // can never leave a marker over a tree it did not come from. Failing here is
    // fatal on purpose — a tree nobody can vouch for would just be re-downloaded
    // by the next build, so burning the download silently is the worse outcome.
    let marker = tmp.join(MARKER_FILE);
    // An archive is free to ship an entry with this name (a directory or a
    // symlink would make the write below fail, or redirect it). Clear the path
    // first so the install's own record is always the one that lands.
    let _ = std::fs::remove_file(&marker);
    let _ = std::fs::remove_dir_all(&marker);
    std::fs::write(&marker, format!("{expected}\n"))
        .map_err(|e| anyhow::anyhow!("failed to record {MARKER_FILE} in the cache entry: {e}"))?;

    // Swap into place. On unix `rename` atomically replaces an existing dest;
    // on Windows rename fails if dest exists, so remove-then-rename there.
    if std::fs::rename(tmp, dest).is_err() {
        let _ = std::fs::remove_dir_all(dest);
        std::fs::rename(tmp, dest)
            .map_err(|e| anyhow::anyhow!("failed to finalize browser cache (rename): {e}"))?;
    }
    eprintln!("[shirabe] installed to {}", dest.display());
    Ok(())
}

/// Fetch `url` with up to 3 attempts (exponential backoff), then verify the body
/// against `expected` (hex sha256) — always, not only when an env var is set.
///
/// Honours two env knobs (besides `SHIRABE_CHROME_MIRROR`, read earlier when
/// composing the URL):
/// - `SHIRABE_DOWNLOAD_PROXY` — route the download through an HTTP/HTTPS/SOCKS
///   proxy, e.g. `http://127.0.0.1:7890` or `socks5://127.0.0.1:1080`. Useful
///   behind the GFW or in CI that can only egress through a forward proxy.
/// - `SHIRABE_DOWNLOAD_TIMEOUT_SECS` — per-request timeout (default 600).
fn fetch_with_retry(url: &str, expected: &str) -> anyhow::Result<Vec<u8>> {
    install_ring_provider();
    let timeout = std::env::var("SHIRABE_DOWNLOAD_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n: &u64| n > 0)
        .map(std::time::Duration::from_secs)
        .unwrap_or(std::time::Duration::from_secs(600));
    let mut builder = reqwest::blocking::Client::builder().timeout(timeout);
    if let Ok(proxy) = std::env::var("SHIRABE_DOWNLOAD_PROXY") {
        let proxy = proxy.trim();
        if !proxy.is_empty() {
            eprintln!("[shirabe] using download proxy {proxy}");
            // A single `all` proxy covers http/https/socks; reqwest picks the
            // right scheme from the URL prefix.
            builder =
                builder.proxy(reqwest::Proxy::all(proxy).map_err(|e| {
                    anyhow::anyhow!("invalid SHIRABE_DOWNLOAD_PROXY {proxy:?}: {e}")
                })?);
        }
    }
    let client = builder.build()?;
    let mut last_err: Option<anyhow::Error> = None;
    for attempt in 1..=3 {
        let outcome = client
            .get(url)
            .header("User-Agent", "shirabe")
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.bytes());
        match outcome {
            Ok(b) => {
                let bytes = b.to_vec();
                // Checksum mismatch is deterministic — fail fast, don't retry.
                return verify_checksum(&bytes, expected)
                    .map_err(|e| {
                        eprintln!("[shirabe] checksum failed for {url} (not retrying): {e}");
                        e
                    })
                    .map(|()| bytes);
            }
            Err(e) => {
                eprintln!("[shirabe] attempt {attempt}: {e}");
                last_err = Some(e.into());
            }
        }
        if attempt < 3 {
            std::thread::sleep(std::time::Duration::from_secs(1u64 << attempt));
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("download failed after 3 attempts")))
}

/// Verify the body against `expected` (lowercase hex). A mismatch is an
/// [`ChecksumError`], which `main` turns into a fatal build abort.
fn verify_checksum(bytes: &[u8], expected: &str) -> anyhow::Result<()> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let actual: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    let expected = expected.trim().to_lowercase();
    if actual != expected {
        return Err(anyhow::Error::new(ChecksumError(format!(
            "checksum mismatch: expected {expected}, got {actual}"
        ))));
    }
    eprintln!("[shirabe] checksum verified");
    Ok(())
}

/// Best-effort sweep of stale temp dirs (from crashed runs) under `parent`,
/// older than `max_age`. Ignores errors. Live downloads have fresh mtimes.
#[allow(dead_code)]
fn sweep_stale_temps(parent: &Path, max_age: std::time::Duration) {
    let Ok(entries) = std::fs::read_dir(parent) else {
        return;
    };
    let cutoff = std::time::SystemTime::now() - max_age;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.starts_with('.') || !name.ends_with(".tmp") {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            if meta.is_dir() {
                if let Ok(mtime) = meta.modified() {
                    if mtime < cutoff {
                        let _ = std::fs::remove_dir_all(entry.path());
                    }
                }
            }
        }
    }
}

/// Per-call nonce making each download's temp dir unique.
static TMP_NONCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn extract_zip(bytes: &[u8], dest: &Path) -> anyhow::Result<()> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();
        // Capture unix mode before the mutable borrow in io::copy; unused off-unix.
        #[cfg(unix)]
        let unix_mode = entry.unix_mode();
        // Strip traversal / absolute components from archive entry names.
        let mut safe = PathBuf::new();
        for comp in Path::new(&name).components() {
            use std::path::Component::*;
            if let Normal(c) = comp {
                safe.push(c);
            }
        }
        let path = dest.join(safe);
        if name.ends_with('/') {
            std::fs::create_dir_all(&path)?;
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Recreate symlinks (macOS .app bundles ship internal symlinks).
        #[cfg(unix)]
        let is_symlink = unix_mode.is_some_and(|m| (m & 0o170000) == 0o120000);
        #[cfg(not(unix))]
        let is_symlink = false;
        if is_symlink {
            #[cfg(unix)]
            {
                use std::io::Read;
                let mut target = String::new();
                let _ = entry.read_to_string(&mut target);
                let target = target.trim();
                if !target.is_empty() && !target.starts_with('/') && !target.contains("..") {
                    let _ = std::os::unix::fs::symlink(target, &path);
                }
            }
        } else {
            let mut out = std::fs::File::create(&path)?;
            std::io::copy(&mut entry, &mut out)?;
            drop(out);
            // Restore unix permission bits so executables keep their exec bit.
            #[cfg(unix)]
            if let Some(mode) = unix_mode {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode));
            }
        }
    }
    Ok(())
}
