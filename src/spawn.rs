//! Satellite invocation: spawn the binary, pipe the MCP `arguments` JSON to its
//! stdin (the ecosystem-wide stdin-wins contract) and parse the JSON on stdout.
//!
//! Every tool call is capped at 8 s so secforge always answers inside BIT's
//! 10 s MCP client timeout.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context;
use serde_json::Value;
use tokio::process::Command;

use crate::tools::{Satellite, ToolDef};

/// Hard cap per tool call (BIT's MCP client times out at 10 s).
pub const CALL_TIMEOUT: Duration = Duration::from_secs(8);

/// Timeout for the `--version` availability probe.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// Truncate long text for error messages.
fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        return s.to_string();
    }
    let mut end = n;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\u{2026}", &s[..end])
}

/// Locate a binary by name on `PATH` (adds `.exe` on Windows).
pub fn find_on_path(bin: &str) -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        format!("{bin}.exe")
    } else {
        bin.to_string()
    };
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|dir| dir.join(&exe))
        .find(|candidate| candidate.is_file())
}

/// Probe a satellite binary: `<bin> --version` must exit 0 within the timeout.
pub async fn probe(bin: &Path) -> bool {
    let mut cmd = Command::new(bin);
    cmd.arg("--version").stdin(std::process::Stdio::null());
    let run = tokio::time::timeout(PROBE_TIMEOUT, cmd.output()).await;
    matches!(run, Ok(Ok(out)) if out.status.success())
}

/// Resolve the binary path for a satellite: explicit override wins over PATH.
pub async fn resolve(satellite: Satellite, override_path: Option<&PathBuf>) -> Option<PathBuf> {
    if let Some(p) = override_path {
        return probe(p).await.then(|| p.clone());
    }
    let found = find_on_path(satellite.bin_name())?;
    probe(&found).await.then_some(found)
}

/// Invoke one tool on its satellite. Returns the satellite's stdout JSON.
///
/// Errors are plain English strings — they end up as MCP `isError` content.
pub async fn call_satellite(
    bin: &Path,
    tool: &ToolDef,
    arguments: Value,
    scan_authorized: bool,
) -> Result<Value, String> {
    if tool.scan_gated && !scan_authorized {
        return Err(format!(
            "tool '{}' touches other hosts and requires the MCP server to be started with \
             --yes-i-have-permission (or SECFORGE_I_HAVE_PERMISSION=yes). Only scan networks \
             you own or are authorized to test.",
            tool.name
        ));
    }

    let args = if arguments.is_object() {
        arguments
    } else {
        Value::Object(Default::default())
    };

    let mut cmd = Command::new(bin);
    cmd.args(tool.argv)
        .arg("--json")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    if scan_authorized {
        if let Some((key, value)) = tool.satellite.permission_env() {
            cmd.env(key, value);
        }
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("failed to launch {}: {e}", tool.satellite.bin_name()))?;

    // stdin: the MCP arguments JSON (stdin wins over CLI args in every satellite).
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        let payload = serde_json::to_vec(&args).unwrap_or_default();
        let _ = stdin.write_all(&payload).await;
        let _ = stdin.shutdown().await;
    }

    let output = tokio::time::timeout(CALL_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| {
            format!(
                "tool '{}' timed out after {}s (BIT's MCP client timeout is 10s — pass tighter bounds)",
                tool.name,
                CALL_TIMEOUT.as_secs()
            )
        })?
        .map_err(|e| format!("failed to run {}: {e}", tool.satellite.bin_name()))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        let detail = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        let code = output.status.code().unwrap_or(-1);
        return Err(format!(
            "{} exited with code {code}: {}",
            tool.satellite.bin_name(),
            truncate(detail, 400)
        ));
    }

    let stdout = stdout.trim();
    if stdout.is_empty() {
        return Err(format!(
            "{} produced no output{}",
            tool.satellite.bin_name(),
            if stderr.trim().is_empty() {
                String::new()
            } else {
                format!(" (stderr: {})", truncate(stderr.trim(), 200))
            }
        ));
    }

    // Satellites emit one JSON object/line on stdout; take the first non-empty line.
    let line = stdout
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or(stdout);
    serde_json::from_str::<Value>(line).map_err(|e| {
        format!(
            "{} did not return valid JSON ({e}): {}",
            tool.satellite.bin_name(),
            truncate(line, 200)
        )
    })
}

/// Convenience wrapper used by tests and callers holding a resolved path.
pub async fn call_ok(bin: &Path, tool: &ToolDef, arguments: Value) -> anyhow::Result<Value> {
    call_satellite(bin, tool, arguments, true)
        .await
        .map_err(anyhow::Error::msg)
        .with_context(|| format!("calling {}", tool.name))
}
