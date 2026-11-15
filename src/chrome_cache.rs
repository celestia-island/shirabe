//! The Chrome cache trust contract: one marker file, one verdict, compiled
//! verbatim by the two surfaces that populate and consume the cache.
//!
//! - `build.rs` (the build-time resolver under `auto-fetch`) declares it as
//!   `#[path = "src/chrome_cache.rs"] mod chrome_cache;`. A build script is its
//!   own compilation unit and cannot `use crate::…`, but it can compile this
//!   very file.
//! - `crate::browser_fetch` (the runtime resolver and the `runtime-fetch`
//!   installer) uses it as an ordinary module.
//!
//! Sharing the file (rather than restating the rules on each side) is the point:
//! a probe and an install that disagree about the marker either discard a good
//! tree — a ~150 MB re-download — or trust one nobody verified.
//!
//! # The contract
//!
//! A cache entry `<cache>/<flavor>/<version>/<download id>/` counts as verified
//! only while its `.shirabe-verified` marker records the digest a *fresh*
//! install would be held to right now. The extracted bytes are never re-hashed,
//! so that marker is the entire argument for reusing a tree instead of
//! downloading it again. Everything else — no marker (pre-marker cache, a
//! CI-restored cache, a cached Docker layer), an unreadable marker, a different
//! digest — is untrusted, and the tree is replaced by a fresh verified install
//! rather than used on the strength of "it is already on disk".
//!
//! Recording a marker is only honest when the caller knows the digest it
//! verified against; with no digest there is nothing to write, and installs stay
//! unmarked (the pre-marker behaviour). What a *failed* write means is the
//! install's call, and the two deliberately differ: `build.rs` treats it as
//! fatal (a build must not bake a path to a tree it cannot vouch for), while the
//! runtime install downgrades it to a warning (the bytes it just verified are
//! usable; an unmarked tree merely fails closed on the next run, as an unmarked
//! re-download).
//!
//! KEEP IN SYNC: the entry root is computed in two places —
//! `build.rs::version_dir` and `browser_fetch::version_dir` — and both must
//! yield `<cache>/<flavor>/<version>/<download id>`, or the marker is written
//! somewhere nobody reads.

use std::path::Path;

/// Install-time marker inside a cache entry: it holds the (lowercase hex) digest
/// the extracted tree was verified against, and is the only thing that makes a
/// later build or runtime resolution trust that tree without downloading it
/// again.
pub(crate) const MARKER_FILE: &str = ".shirabe-verified";

/// What the install-time marker says about a tree that is already on disk.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum MarkerVerdict {
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
pub(crate) fn classify_marker(marker: Option<&str>, expected: &str) -> MarkerVerdict {
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

/// Read the marker at `path` and classify it. Any read failure other than
/// "absent" is also "untrusted": a marker we cannot read is a marker we cannot
/// use as evidence.
pub(crate) fn read_marker(path: &Path, expected: &str) -> MarkerVerdict {
    match std::fs::read_to_string(path) {
        Ok(contents) => classify_marker(Some(&contents), expected),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => classify_marker(None, expected),
        Err(e) => MarkerVerdict::Untrusted(format!("{} is unreadable ({e})", path.display())),
    }
}

/// Record `expected` as the provenance of the cache entry rooted at `dir`, i.e.
/// write `<dir>/.shirabe-verified` in canonical form (trimmed, lowercase hex).
///
/// Callers write this *inside* the temp tree and then swap that tree into the
/// cache, so the marker always arrives together with the bytes it vouches for:
/// an interrupted install can never leave a marker over a tree it did not come
/// from. An archive is free to ship an entry with this name (a directory or a
/// symlink would make the write fail, or redirect it), so the path is cleared
/// first and the install's own record is always the one that lands.
///
/// Only the digest the caller verified against belongs here. Callers decide
/// whether there is one at all; this helper writes what it is given.
// Only the `runtime-fetch` install path writes markers from the library (the
// cache probe, which every build compiles, only reads them). Build scripts never
// see `cfg(feature = …)`, but `build.rs` calls this function, so the `allow` is
// inert there — a real dead-code warning would still surface on either side.
#[cfg_attr(not(feature = "runtime-fetch"), allow(dead_code))]
pub(crate) fn record_marker(dir: &Path, expected: &str) -> std::io::Result<()> {
    let marker = dir.join(MARKER_FILE);
    let _ = std::fs::remove_file(&marker);
    let _ = std::fs::remove_dir_all(&marker);
    std::fs::write(&marker, format!("{}\n", expected.trim().to_lowercase()))
}

// ── the pinned archive table (shared by the build gate and the runtime) ──
//
// Living here rather than in `build.rs` is what lets the RUNTIME apply the
// same default the build applies: one table, compiled into both crates (see
// `build.rs`'s `#[path] mod chrome_cache`). Before this move the runtime only
// verified when `SHIRABE_CHROME_SHA256` was set, so a runtime-installed tree
// was trusted on trust alone by the next build.

/// The Chrome-for-Testing release the table below pins.
pub const CHROME_VERSION_PINNED: &str = "150.0.7871.46";

/// SHA-256 of the pinned Chrome-for-Testing archives, keyed by
/// `(version, flavor, download id)`.
///
/// ⚠️ Bumping `CHROME_VERSION_PINNED` — or changing which flavors / download
/// ids are supported — REQUIRES refreshing this table in the same commit:
/// fetch every archive from the default mirror, `sha256sum` it, paste the
/// digests. A stale entry fails loudly (build gate) or, at runtime, triggers a
/// re-download that fails and then falls back to the on-disk tree with a
/// warning (see `browser_fetch::ensure`).
pub const CHROME_SHA256: &[(&str, &str, &str, &str)] = &[
    (
        CHROME_VERSION_PINNED,
        "shell",
        "linux64",
        "0395e5db8d1631d5ed879d4f05fec423425537018fa1d73a58b1ade13288f447",
    ),
    (
        CHROME_VERSION_PINNED,
        "shell",
        "mac-arm64",
        "38d663dcf205c8fc210cde9238f4b7f55c5f3757ec89f325ce0ec53951a842e7",
    ),
    (
        CHROME_VERSION_PINNED,
        "shell",
        "mac-x64",
        "50b42d8313d44dc38c696fc2851e8c7dd2fd95b3dd69e05134abdab143d813c5",
    ),
    (
        CHROME_VERSION_PINNED,
        "shell",
        "win64",
        "b97b40bbad365db24c69c00d3150c9dfd10012d16eb5da82e1fb73984a7b7f88",
    ),
    (
        CHROME_VERSION_PINNED,
        "full",
        "linux64",
        "ad115a7498a17f53f6ed0914458326c6516addc756224db14c32184a9b1ab078",
    ),
    (
        CHROME_VERSION_PINNED,
        "full",
        "mac-arm64",
        "9b6c6fbbda6f2948c74425828bd18f0e59542fd2beb056697478c38a956d83cb",
    ),
    (
        CHROME_VERSION_PINNED,
        "full",
        "mac-x64",
        "dc6280df5fa5cb1da96f7026a20d7622df921fa5ba03c07e95af4a8dde17b131",
    ),
    (
        CHROME_VERSION_PINNED,
        "full",
        "win64",
        "2245f764e024897195bce1cf6ff16d60544351e4d1b68edb6956ea875ab8b929",
    ),
];

/// Pinned digest for `(version, flavor, download id)`; `None` = not pinned.
pub fn pinned_sha256(ver: &str, flavor: &str, id: &str) -> Option<&'static str> {
    let ver = ver.trim();
    CHROME_SHA256
        .iter()
        .find(|(v, f, i, _)| *v == ver && *f == flavor && *i == id)
        .map(|(_, _, _, digest)| *digest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Unique scratch directory under the system temp dir (no `tempfile` dep).
    fn scratch_dir() -> std::path::PathBuf {
        static NONCE: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "shirabe-chrome-marker-{}-{}",
            std::process::id(),
            NONCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    /// Unwrap the reason out of an untrusted verdict (a `Verified` here is a bug).
    fn reason(verdict: MarkerVerdict) -> String {
        match verdict {
            MarkerVerdict::Verified => panic!("expected an untrusted verdict, got Verified"),
            MarkerVerdict::Untrusted(why) => why,
        }
    }

    #[test]
    fn matching_marker_is_verified() {
        assert_eq!(
            classify_marker(Some("abc123"), "abc123"),
            MarkerVerdict::Verified
        );
        // Neither whitespace nor case is part of the digest: an install that
        // wrote a trailing newline (or uppercase hex) still matches.
        assert_eq!(
            classify_marker(Some(" ABC123\n"), "abc123"),
            MarkerVerdict::Verified
        );
    }

    #[test]
    fn absent_marker_is_untrusted() {
        let why = reason(classify_marker(None, "abc123"));
        assert!(why.contains(MARKER_FILE), "{why}");
    }

    #[test]
    fn empty_marker_is_untrusted() {
        let why = reason(classify_marker(Some("  \n"), "abc123"));
        assert!(why.contains("empty"), "{why}");
    }

    #[test]
    fn other_digest_is_untrusted() {
        let why = reason(classify_marker(Some("deadbeef\n"), "abc123"));
        // Both digests appear so the message explains the rejection on its own.
        assert!(why.contains("deadbeef"), "{why}");
        assert!(why.contains("abc123"), "{why}");
    }

    #[test]
    fn record_then_read_roundtrip() {
        let dir = scratch_dir();
        let marker = dir.join(MARKER_FILE);
        assert!(matches!(
            read_marker(&marker, "abc"),
            MarkerVerdict::Untrusted(_)
        ));

        record_marker(&dir, "ABC").expect("record marker");
        // Stored canonically, so the file is stable across installs.
        assert_eq!(std::fs::read_to_string(&marker).unwrap(), "abc\n");
        assert_eq!(read_marker(&marker, "abc"), MarkerVerdict::Verified);

        // Reinstalling the same entry replaces the record, never appends to it.
        record_marker(&dir, "def").expect("re-record marker");
        assert_eq!(std::fs::read_to_string(&marker).unwrap(), "def\n");
        assert!(matches!(
            read_marker(&marker, "abc"),
            MarkerVerdict::Untrusted(_)
        ));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn record_clears_an_archive_shipped_entry_of_the_same_name() {
        let dir = scratch_dir();
        // An archive may ship `.shirabe-verified`; a directory at that path would
        // make a plain write fail, so the helper clears it first.
        std::fs::create_dir_all(dir.join(MARKER_FILE)).expect("plant directory");
        record_marker(&dir, "abc").expect("record over the planted directory");
        assert_eq!(
            read_marker(&dir.join(MARKER_FILE), "abc"),
            MarkerVerdict::Verified
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
