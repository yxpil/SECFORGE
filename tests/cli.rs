//! Integration tests running the real `secforge` binary (CLI surface).

use std::io::Read;
use std::process::{Command, Stdio};

use serde_json::Value;

struct Run {
    status: i32,
    stdout: String,
    stderr: String,
}

fn run(args: &[&str]) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_secforge"))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to spawn secforge");
    Run {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn parse(result: &Run) -> Value {
    serde_json::from_str(result.stdout.trim())
        .unwrap_or_else(|err| panic!("stdout is not valid JSON ({err}): {}", result.stdout))
}

#[test]
fn help_exits_zero_and_documents_the_mcp_endpoint() {
    let result = run(&["--help"]);
    assert_eq!(result.status, 0, "stderr: {}", result.stderr);
    for text in [
        "serve",
        "tools",
        "--yes-i-have-permission",
        "--neton",
        "/mcp",
        "8756",
    ] {
        assert!(result.stdout.contains(text), "help should mention '{text}'");
    }
}

#[test]
fn no_command_prints_help_and_exits_zero() {
    let result = run(&[]);
    assert_eq!(result.status, 0, "stderr: {}", result.stderr);
    assert!(result.stdout.contains("Usage"));
}

#[test]
fn version_prints_semver() {
    let result = run(&["--version"]);
    assert_eq!(result.status, 0);
    assert!(
        result.stdout.contains("secforge 0.1.0"),
        "stdout: {}",
        result.stdout
    );
}

#[test]
fn tools_command_outputs_json_shape() {
    // Point --neton at the secforge binary itself: `secforge --version` exits 0,
    // so the probe passes and the neton tools get exposed. Deterministic on any
    // machine and any platform.
    let self_bin = env!("CARGO_BIN_EXE_secforge");
    let result = run(&["tools", "--neton", self_bin]);
    assert_eq!(result.status, 0, "stderr: {}", result.stderr);
    let value = parse(&result);
    assert!(value["scan_authorized"].is_boolean());
    assert_eq!(value["satellites"]["neton"], self_bin);
    let tools = value["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), 11, "11 neton tools exposed: {tools:?}");
    for tool in tools {
        assert!(tool["name"].as_str().unwrap().starts_with("neton_"));
        assert!(tool["description"].is_string());
        assert_eq!(tool["inputSchema"]["type"], "object");
    }
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"neton_netscan"));
    assert!(names.contains(&"neton_info"));
}

#[test]
fn tools_command_without_satellites_exposes_nothing() {
    // PATH without any satellite: spawn with a cleared environment PATH.
    let output = Command::new(env!("CARGO_BIN_EXE_secforge"))
        .args(["tools"])
        .env("PATH", "")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn");
    assert_eq!(output.status.code(), Some(0));
    let value: Value = serde_json::from_slice(&output.stdout).expect("json");
    assert_eq!(value["tools"].as_array().unwrap().len(), 0);
}

#[test]
fn serve_answers_health_and_mcp_handshake_on_random_port() {
    let self_bin = env!("CARGO_BIN_EXE_secforge");
    // Pick a free port by binding and releasing it.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    drop(listener);

    let mut child = Command::new(self_bin)
        .args(["serve", "--port", &port.to_string(), "--neton", self_bin])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stderr = String::new();
    // Wait for readiness by watching the stderr startup line.
    let mut stderr_pipe = child.stderr.take().expect("stderr");
    let mut ready = false;
    for _ in 0..50 {
        let mut buf = [0u8; 512];
        if let Ok(n) = stderr_pipe.read(&mut buf) {
            if n > 0 {
                stderr.push_str(&String::from_utf8_lossy(&buf[..n]));
            }
        }
        if stderr.contains("listening on") {
            ready = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    // Ensure the pipe keeps draining in the background after readiness.
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr_pipe.read_to_end(&mut buf);
    });
    assert!(ready, "server never became ready; stderr: {stderr}");

    // /health
    let health = ureq::get(&format!("http://127.0.0.1:{port}/health"))
        .call()
        .expect("health");
    let body: Value = health.into_json().expect("json");
    assert_eq!(body["ok"], true);
    assert_eq!(body["service"], "secforge");
    assert!(body["tools"].as_u64().expect("tools count") >= 11);

    // MCP handshake over the root path (BIT discovery probes the root).
    let resp = ureq::post(&format!("http://127.0.0.1:{port}/"))
        .send_json(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}
        }))
        .expect("initialize");
    assert_eq!(resp.status(), 200);
    let session = resp
        .header("mcp-session-id")
        .expect("session header")
        .to_string();
    assert!(session.starts_with("mcp-"), "session id: {session}");
    let body: Value = resp.into_json().expect("json");
    assert_eq!(body["result"]["serverInfo"]["name"], "secforge");
    assert_eq!(body["result"]["protocolVersion"], "2025-03-26");

    child.kill().expect("kill serve");
    child.wait().expect("reap serve");
}
