//! End-to-end MCP protocol tests against the in-process router, backed by a
//! fake satellite binary compiled on the fly with `rustc` (cross-platform).
//!
//! The fake satellite implements the same observable contract as the real
//! ones: `--version` exits 0, arguments JSON arrives on stdin, JSON goes to
//! stdout, failures exit non-zero.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::OnceLock;

use serde_json::{json, Value};

use secforge::state::AppState;
use secforge::tools::Satellite;

/// Source of the fake satellite. Behaviour:
/// - `--version` → prints a version line, exit 0 (availability probe).
/// - stdin containing `__fail` → stderr log + exit 1 (error path).
/// - otherwise → `{"fake":true,"subcommand":<argv1>,"got_marker":<stdin has hello-marker>,"perm_env":<NETON_I_HAVE_PERMISSION=yes>}`.
const FAKE_SOURCE: &str = r#"
use std::io::Read;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 && args[1] == "--version" {
        println!("fake-satellite 0.1.0");
        return;
    }
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    if input.contains("__fail") {
        eprintln!("boom: fake failure");
        std::process::exit(1);
    }
    let sub = args.get(1).cloned().unwrap_or_default();
    let got_marker = input.contains("hello-marker");
    let perm_env = std::env::var("NETON_I_HAVE_PERMISSION").as_deref() == Ok("yes");
    println!("{{\"fake\":true,\"subcommand\":\"{sub}\",\"got_marker\":{got_marker},\"perm_env\":{perm_env}}}");
}
"#;

/// Compile the fake satellite once per test run; returns the executable path.
fn fake_satellite() -> &'static PathBuf {
    static FAKE: OnceLock<PathBuf> = OnceLock::new();
    FAKE.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("secforge-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let src = dir.join("fake_satellite.rs");
        std::fs::write(&src, FAKE_SOURCE).expect("write fake source");
        let exe = dir.join(if cfg!(windows) {
            "fake_satellite.exe"
        } else {
            "fake_satellite"
        });
        let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
        let status = std::process::Command::new(rustc)
            .arg("--edition")
            .arg("2021")
            .arg("-o")
            .arg(&exe)
            .arg(&src)
            .status()
            .expect("failed to run rustc — a Rust toolchain must be on PATH to run these tests");
        assert!(
            status.success(),
            "rustc failed to compile the fake satellite"
        );
        exe
    })
}

/// Spawn the MCP router in-process on a random port.
fn spawn_server(scan_authorized: bool) -> SocketAddr {
    let fake = fake_satellite().clone();
    let bins: HashMap<Satellite, PathBuf> = [Satellite::Neton, Satellite::Firelin]
        .into_iter()
        .map(|s| (s, fake.clone()))
        .collect();
    let state = AppState {
        scan_authorized,
        bins,
    };

    let (tx, rx) = mpsc::channel::<SocketAddr>();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind");
            let addr = listener.local_addr().expect("addr");
            tx.send(addr).expect("send addr");
            axum::serve(listener, secforge::mcp::mcp_router(state))
                .await
                .expect("serve");
        });
    });
    rx.recv().expect("server address")
}

/// POST one JSON-RPC message; returns (status, Mcp-Session-Id, parsed body).
fn rpc(base: &str, path: &str, body: Value, session: Option<&str>) -> (u16, Option<String>, Value) {
    let mut req = ureq::post(&format!("{base}{path}"));
    if let Some(sid) = session {
        req = req.set("Mcp-Session-Id", sid);
    }
    let resp = req.send_json(body).expect("request");
    let status = resp.status();
    let session = resp.header("mcp-session-id").map(|s| s.to_string());
    let body = resp.into_json().expect("json body");
    (status, session, body)
}

fn initialize(base: &str, path: &str) -> String {
    let (status, session, body) = rpc(
        base,
        path,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "BIT", "version": "0"}}
        }),
        None,
    );
    assert_eq!(status, 200);
    let sid = session.expect("initialize must return a session id");
    assert!(sid.starts_with("mcp-"), "session id: {sid}");
    assert_eq!(body["result"]["serverInfo"]["name"], "secforge");
    assert_eq!(body["result"]["protocolVersion"], "2025-03-26");
    assert_eq!(
        body["result"]["capabilities"]["tools"]["listChanged"],
        false
    );
    sid
}

#[test]
fn full_flow_initialize_tools_list_and_call() {
    let addr = spawn_server(false);
    let base = format!("http://{addr}");

    // 1. Handshake on the root path (BIT discovery probes the root).
    let sid = initialize(&base, "/");

    // 2. initialized notification (BIT sends it with an id) → 202, empty body.
    let resp = ureq::post(&format!("{base}/mcp"))
        .set("Mcp-Session-Id", &sid)
        .send_json(json!({"jsonrpc": "2.0", "id": 99, "method": "notifications/initialized"}))
        .expect("notification");
    assert_eq!(resp.status(), 202);
    assert!(resp.into_string().unwrap_or_default().is_empty());

    // 2b. Id-less message is also treated as a notification.
    let resp = ureq::post(&format!("{base}/mcp"))
        .set("Mcp-Session-Id", &sid)
        .send_json(json!({"jsonrpc": "2.0", "method": "notifications/cancelled"}))
        .expect("notification");
    assert_eq!(resp.status(), 202);

    // 3. tools/list over /mcp with the session id → all 11 neton + 5 firelin tools.
    let (status, _, body) = rpc(
        &base,
        "/mcp",
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
        Some(&sid),
    );
    assert_eq!(status, 200);
    let tools = body["result"]["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), 16, "neton(11) + firelin(5): {tools:?}");
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"neton_arp"));
    assert!(names.contains(&"firelin_portscan"));
    for tool in tools {
        assert!(tool["inputSchema"]["type"] == "object");
    }
    let netscan = tools
        .iter()
        .find(|t| t["name"] == "neton_netscan")
        .expect("netscan tool");
    assert_eq!(
        netscan["inputSchema"]["properties"]["cidr"]["type"],
        "string"
    );
    assert_eq!(netscan["inputSchema"]["required"][0], "cidr");

    // 4. tools/call: arguments JSON must reach the satellite via stdin.
    let (status, _, body) = rpc(
        &base,
        "/mcp",
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
               "params": {"name": "neton_info", "arguments": {"hello-marker": 1}}}),
        Some(&sid),
    );
    assert_eq!(status, 200);
    assert_eq!(body["result"]["isError"], false);
    let text = body["result"]["content"][0]["text"]
        .as_str()
        .expect("text content");
    let payload: Value = serde_json::from_str(text).expect("satellite JSON");
    assert_eq!(payload["fake"], true);
    assert_eq!(payload["subcommand"], "info");
    assert_eq!(
        payload["got_marker"], true,
        "arguments JSON must be piped to stdin"
    );
    assert_eq!(
        payload["perm_env"], false,
        "no permission env for non-scan calls"
    );
}

#[test]
fn scan_gate_locked_and_unlocked() {
    // Locked server: scan tool refuses with an isError result (HTTP still 200).
    let locked = spawn_server(false);
    let base = format!("http://{locked}");
    let sid = initialize(&base, "/mcp");
    let (status, _, body) = rpc(
        &base,
        "/mcp",
        json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call",
               "params": {"name": "neton_netscan", "arguments": {"cidr": "192.168.1.0/24"}}}),
        Some(&sid),
    );
    assert_eq!(status, 200);
    assert_eq!(body["result"]["isError"], true);
    let text = body["result"]["content"][0]["text"]
        .as_str()
        .expect("error text");
    assert!(text.contains("--yes-i-have-permission"), "text: {text}");

    // Unlocked server: the scan call goes through and carries the permission env.
    let unlocked = spawn_server(true);
    let base = format!("http://{unlocked}");
    let sid = initialize(&base, "/mcp");
    let (status, _, body) = rpc(
        &base,
        "/mcp",
        json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call",
               "params": {"name": "neton_netscan", "arguments": {"cidr": "192.168.1.0/24"}}}),
        Some(&sid),
    );
    assert_eq!(status, 200);
    assert_eq!(body["result"]["isError"], false);
    let payload: Value =
        serde_json::from_str(body["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(payload["subcommand"], "netscan");
    assert_eq!(
        payload["perm_env"], true,
        "NETON_I_HAVE_PERMISSION must reach the child"
    );
}

#[test]
fn satellite_failure_becomes_is_error_result() {
    let addr = spawn_server(false);
    let base = format!("http://{addr}");
    let sid = initialize(&base, "/mcp");
    let (status, _, body) = rpc(
        &base,
        "/mcp",
        json!({"jsonrpc": "2.0", "id": 6, "method": "tools/call",
               "params": {"name": "neton_dns", "arguments": {"__fail": true}}}),
        Some(&sid),
    );
    assert_eq!(status, 200, "tool failures must stay HTTP 200");
    assert_eq!(body["result"]["isError"], true);
    let text = body["result"]["content"][0]["text"]
        .as_str()
        .expect("error text");
    assert!(text.contains("exited with code 1"), "text: {text}");
    assert!(text.contains("boom"), "stderr must be surfaced: {text}");
}

#[test]
fn protocol_errors_ping_and_notifications() {
    let addr = spawn_server(false);
    let base = format!("http://{addr}");
    let sid = initialize(&base, "/");

    // ping → empty result object.
    let (status, _, body) = rpc(
        &base,
        "/mcp",
        json!({"jsonrpc": "2.0", "id": 7, "method": "ping", "params": {}}),
        Some(&sid),
    );
    assert_eq!(status, 200);
    assert_eq!(body["result"], json!({}));

    // Unknown tool → -32602.
    let (_, _, body) = rpc(
        &base,
        "/mcp",
        json!({"jsonrpc": "2.0", "id": 8, "method": "tools/call", "params": {"name": "nope"}}),
        Some(&sid),
    );
    assert_eq!(body["error"]["code"], -32602);

    // Unknown method → -32601.
    let (_, _, body) = rpc(
        &base,
        "/mcp",
        json!({"jsonrpc": "2.0", "id": 9, "method": "bogus/x"}),
        Some(&sid),
    );
    assert_eq!(body["error"]["code"], -32601);

    // Malformed JSON body → -32700.
    let resp = ureq::post(&format!("{base}/mcp"))
        .set("content-type", "application/json")
        .send_string("{not json")
        .expect("request");
    let body: Value = resp.into_json().expect("json");
    assert_eq!(body["error"]["code"], -32700);

    // Version negotiation: unknown client version falls back to the latest.
    let (_, _, body) = rpc(
        &base,
        "/mcp",
        json!({"jsonrpc": "2.0", "id": 10, "method": "initialize",
               "params": {"protocolVersion": "1999-01-01", "capabilities": {}, "clientInfo": {"name": "x"}}}),
        None,
    );
    assert_eq!(body["result"]["protocolVersion"], "2025-06-18");
}

#[test]
fn hostile_argument_values_are_passed_as_literal_stdin_data_not_shelled() {
    // Command-injection attempt in an argument *value*: secforge pipes the whole
    // arguments object to the child's stdin as JSON (never via a shell), so shell
    // metacharacters must ride along as an opaque string and never execute.
    let addr = spawn_server(false);
    let base = format!("http://{addr}");
    let sid = initialize(&base, "/mcp");

    let sentinel = "pwned_secforge_marker.txt";
    let _ = std::fs::remove_file(sentinel);
    let payload = format!("\"; touch {sentinel}; $(whoami) #");
    let (status, _, body) = rpc(
        &base,
        "/mcp",
        json!({"jsonrpc": "2.0", "id": 20, "method": "tools/call",
               "params": {"name": "neton_info", "arguments": {"hello-marker": 1, "host": payload}}}),
        Some(&sid),
    );
    assert_eq!(status, 200);
    assert_eq!(body["result"]["isError"], false, "hostile args must still be a normal call: {body}");
    let text = body["result"]["content"][0]["text"].as_str().expect("text");
    let out: Value = serde_json::from_str(text).expect("satellite JSON");
    assert_eq!(out["fake"], true);
    assert_eq!(out["got_marker"], true, "arguments reached the child on stdin as data");
    // The shell command in the payload must NOT have been executed.
    assert!(
        !std::path::Path::new(sentinel).exists(),
        "injection payload executed a shell command!"
    );
    let _ = std::fs::remove_file(sentinel);
}

#[test]
fn non_object_and_garbage_arguments_are_handled_without_panic() {
    let addr = spawn_server(false);
    let base = format!("http://{addr}");
    let sid = initialize(&base, "/mcp");

    // arguments as a bare string (not an object): call_satellite must coerce it
    // to an empty object and run the child cleanly, never panic.
    let (status, _, body) = rpc(
        &base,
        "/mcp",
        json!({"jsonrpc": "2.0", "id": 21, "method": "tools/call",
               "params": {"name": "neton_info", "arguments": "totally-not-an-object"}}),
        Some(&sid),
    );
    assert_eq!(status, 200);
    assert_eq!(body["result"]["isError"], false, "garbage args must not crash the router: {body}");

    // Unknown tool with a wildly wrong argument type → clean -32602, no panic.
    let (_, _, body) = rpc(
        &base,
        "/mcp",
        json!({"jsonrpc": "2.0", "id": 22, "method": "tools/call",
               "params": {"name": "definitely_not_a_tool", "arguments": [[[[[1]]]]]}}),
        Some(&sid),
    );
    assert_eq!(body["error"]["code"], -32602, "unknown tool must be a clean protocol error: {body}");
}
