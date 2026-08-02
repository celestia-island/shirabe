//! Cross-process tab lease.
//!
//! shirabe drives one headless browser per process, and each browser owns a
//! single page — so "tabs" in the resource-limit sense are processes. To keep
//! a swarm of `shirabe mcp` sessions (or debug servers) from eating the
//! machine, every spawned browser must first acquire a slot from a
//! machine-wide lease file.
//!
//! The lease file is a JSONL heartbeat table: one line per live browser, each
//! carrying a unique id plus the Unix-epoch milliseconds of its last
//! heartbeat. A watchdog refreshes the heartbeat every couple of seconds;
//! entries whose heartbeat is stale are treated as dead and evicted, so a
//! crashed (or SIGKILLed) shirabe releases its slot automatically. The cap is
//! advisory — a race can briefly overshoot by one — which is fine for a
//! guardrail that exists to *warn* before resources run out.
//!
//! Knobs:
//! - `SHIRABE_MAX_TABS` — cap on concurrent browsers (default 3).
//! - `SHIRABE_LEASE_FILE` — lease file location (default
//!   `{temp_dir}/shirabe-tabs.lease.jsonl`); mainly for tests.

use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Disambiguates lease ids taken in the same process within the same
/// millisecond (pid + timestamp alone can collide under fast re-acquires).
static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Default concurrent-browser cap.
const DEFAULT_MAX_TABS: usize = 3;
/// A lease entry whose heartbeat is older than this is considered dead.
const STALE_AFTER: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    id: String,
    /// Unix-epoch milliseconds of the last heartbeat.
    hb: u64,
}

/// The machine-wide cap on concurrent shirabe browsers (`SHIRABE_MAX_TABS`).
pub fn max_tabs() -> usize {
    std::env::var("SHIRABE_MAX_TABS")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_MAX_TABS)
}

/// Number of currently-live lease entries across all processes.
pub fn used_slots() -> usize {
    live_entries(&lease_path()).len()
}

/// `(cap, used, remaining)` snapshot for `/info` and the MCP tools.
pub fn snapshot() -> (usize, usize, usize) {
    let cap = max_tabs();
    let used = used_slots();
    (cap, used, cap.saturating_sub(used))
}

fn lease_path() -> PathBuf {
    std::env::var_os("SHIRABE_LEASE_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("shirabe-tabs.lease.jsonl"))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn read_entries(path: &PathBuf) -> Vec<Entry> {
    let mut out = Vec::new();
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return out,
    };
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(e) = serde_json::from_str::<Entry>(line) {
            out.push(e);
        }
    }
    out
}

fn live_entries(path: &PathBuf) -> Vec<Entry> {
    let now = now_ms();
    let stale_ms = STALE_AFTER.as_millis() as u64;
    read_entries(path)
        .into_iter()
        .filter(|e| now.saturating_sub(e.hb) < stale_ms)
        .collect()
}

fn write_entries(path: &PathBuf, entries: &[Entry]) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)?;
    for e in entries {
        writeln!(file, "{}", serde_json::to_string(e).unwrap_or_default())?;
    }
    file.flush()
}

/// A held tab slot. Dropping it (or letting it die) releases the slot.
#[derive(Debug, Clone)]
pub struct TabLease {
    id: String,
    path: PathBuf,
}

impl TabLease {
    /// Try to take a slot. Fails with a guidance message when the cap is
    /// reached and `override_limit` is false.
    pub fn acquire(override_limit: bool) -> Result<Self, String> {
        let path = lease_path();
        let cap = max_tabs();
        let mut entries = live_entries(&path);
        let used = entries.len();
        if used >= cap && !override_limit {
            return Err(format!(
                "tab cap reached ({used}/{cap}): pass override=true to exceed the limit, \
                 or close other shirabe browsers via the `browser_close` tool / \
                 POST /browser/close"
            ));
        }
        let mine = Entry {
            id: format!(
                "{}-{}-{}",
                std::process::id(),
                now_ms(),
                ID_COUNTER.fetch_add(1, Ordering::SeqCst)
            ),
            hb: now_ms(),
        };
        entries.push(mine.clone());
        write_entries(&path, &entries)
            .map_err(|e| format!("failed to write tab lease file: {e}"))?;
        Ok(Self { id: mine.id, path })
    }

    /// Refresh this slot's heartbeat (called by the browser watchdog).
    pub fn refresh(&self) {
        let mut entries = read_entries(&self.path);
        if let Some(mine) = entries.iter_mut().find(|e| e.id == self.id) {
            mine.hb = now_ms();
        } else {
            entries.push(Entry {
                id: self.id.clone(),
                hb: now_ms(),
            });
        }
        let _ = write_entries(&self.path, &entries);
    }
}

impl Drop for TabLease {
    fn drop(&mut self) {
        let entries = read_entries(&self.path)
            .into_iter()
            .filter(|e| e.id != self.id)
            .collect::<Vec<_>>();
        let _ = write_entries(&self.path, &entries);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run a closure with an isolated lease file in a fresh temp dir.
    fn with_lease<R>(f: impl FnOnce() -> R) -> R {
        let dir = std::env::temp_dir().join(format!(
            "shirabe-lease-test-{}-{:x}",
            std::process::id(),
            now_ms()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("lease.jsonl");
        // SAFETY: tests are run single-threaded for env mutation under the
        // `serial` lock; no other thread reads these vars concurrently.
        unsafe { std::env::set_var("SHIRABE_LEASE_FILE", &file) };
        let r = f();
        unsafe { std::env::remove_var("SHIRABE_LEASE_FILE") };
        let _ = std::fs::remove_dir_all(&dir);
        r
    }

    fn set_max(n: usize) {
        // SAFETY: guarded by `serial` — see `with_lease`.
        unsafe { std::env::set_var("SHIRABE_MAX_TABS", n.to_string()) };
    }

    fn clear_max() {
        // SAFETY: guarded by `serial` — see `with_lease`.
        unsafe { std::env::remove_var("SHIRABE_MAX_TABS") };
    }

    #[test]
    #[serial_test::serial]
    fn acquire_honors_cap_and_override() {
        with_lease(|| {
            set_max(2);
            let a = TabLease::acquire(false).unwrap();
            let b = TabLease::acquire(false).unwrap();
            let denied = TabLease::acquire(false).unwrap_err();
            assert!(denied.contains("cap reached"), "denial: {denied}");
            assert!(denied.contains("override"), "denial: {denied}");
            // override_limit bypasses the cap.
            let c = TabLease::acquire(true).unwrap();
            assert_eq!(used_slots(), 3);
            drop(c);
            assert_eq!(used_slots(), 2);
            drop(b);
            drop(a);
            assert_eq!(used_slots(), 0);
            clear_max();
        });
    }

    #[test]
    #[serial_test::serial]
    fn drop_releases_slot() {
        with_lease(|| {
            set_max(1);
            {
                let a = TabLease::acquire(false).unwrap();
                assert_eq!(used_slots(), 1);
                drop(a);
            }
            assert_eq!(used_slots(), 0);
            // Re-acquire after release succeeds.
            let _b = TabLease::acquire(false).unwrap();
            clear_max();
        });
    }

    #[test]
    #[serial_test::serial]
    fn stale_entries_are_evicted() {
        with_lease(|| {
            set_max(1);
            // Simulate a dead browser: write a lease whose heartbeat is ancient.
            let path = lease_path();
            let ghost = Entry {
                id: "ghost".into(),
                hb: now_ms() - STALE_AFTER.as_millis() as u64 * 2,
            };
            write_entries(&path, &[ghost]).unwrap();
            assert_eq!(used_slots(), 0, "stale ghost must not block a slot");
            let a = TabLease::acquire(false).unwrap();
            assert_eq!(used_slots(), 1);
            drop(a);
            clear_max();
        });
    }

    #[test]
    #[serial_test::serial]
    fn refresh_keeps_slot_live() {
        with_lease(|| {
            set_max(1);
            let a = TabLease::acquire(false).unwrap();
            a.refresh();
            assert_eq!(used_slots(), 1);
            drop(a);
            clear_max();
        });
    }
}
