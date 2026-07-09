//! Stdio smoke test for the `shirabe mcp` MCP server.
//!
//! Boots the server as a subprocess, performs the `initialize` handshake, then
//! issues `tools/list` and asserts the browser tool roster is advertised. It
//! does *not* drive a live browser (that needs Chrome + network); the point is
//! to prove the server starts, speaks the protocol, and exposes the surface.
//!
//! Chrome fetching is disabled so the test stays hermetic — the debug server
//! comes up regardless and reports no browser connected.

#![cfg(feature = "mcp")]

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn shirabe_binary() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_shirabe"))
}

fn write_msg<W: Write>(w: &mut W, id: Option<u64>, method: &str, params: serde_json::Value) {
    let mut msg = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
    });
    if let Some(id) = id {
        msg["id"] = serde_json::json!(id);
    }
    serde_json::to_writer(&mut *w, &msg).unwrap();
    writeln!(&mut *w).unwrap();
    w.flush().unwrap();
}

fn read_response<R: BufRead>(r: &mut R, want_id: u64, timeout: Duration) -> serde_json::Value {
    let deadline = Instant::now() + timeout;
    let mut line = String::new();
    loop {
        if Instant::now() > deadline {
            panic!("timed out waiting for response id={want_id}");
        }
        line.clear();
        let n = r.read_line(&mut line).expect("read line");
        if n == 0 {
            panic!("server closed stdout before responding to id={want_id}");
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v.get("id").and_then(|i| i.as_u64()) == Some(want_id) {
            return v;
        }
    }
}

#[test]
#[allow(clippy::zombie_processes)] // best-effort teardown: try_wait then kill
fn mcp_server_lists_browser_tools() {
    let bin = shirabe_binary();
    // Keep the test hermetic: don't fetch Chrome, don't require a real backend.
    let mut child = Command::new(&bin)
        .arg("mcp")
        .env("SHIRABE_SKIP_BROWSER_FETCH", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn `shirabe mcp` ({bin:?}): {e}"));

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    write_msg(
        &mut stdin,
        Some(1),
        "initialize",
        serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "shirabe-mcp-smoke", "version": "0.0.0" },
        }),
    );
    let init = read_response(&mut stdout, 1, Duration::from_secs(20));
    assert_eq!(
        init["result"]["protocolVersion"].as_str(),
        Some("2024-11-05"),
        "initialize response: {init}"
    );
    assert!(
        init["result"]["capabilities"].get("tools").is_some(),
        "server did not advertise tools capability: {init}"
    );

    write_msg(
        &mut stdin,
        None,
        "notifications/initialized",
        serde_json::json!({}),
    );

    write_msg(&mut stdin, Some(2), "tools/list", serde_json::json!({}));
    let list = read_response(&mut stdout, 2, Duration::from_secs(20));
    let tools: Vec<String> = list["result"]["tools"]
        .as_array()
        .expect("tools is an array")
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();

    for expected in [
        "browser_navigate",
        "browser_navigate_back",
        "browser_navigate_forward",
        "browser_snapshot",
        "browser_dom",
        "browser_screenshot",
        "browser_click",
        "browser_type",
        "browser_press_key",
        "browser_evaluate",
        "browser_console_messages",
        "browser_resize",
    ] {
        assert!(
            tools.iter().any(|t| t == expected),
            "missing tool `{expected}`; got: {tools:?}"
        );
    }

    drop(stdin);
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if child.try_wait().map(|o| o.is_some()).unwrap_or(false) {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
}
