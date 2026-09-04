//! Static registry of MCP tools backed by the BITECO satellite binaries.
//!
//! Each tool maps 1:1 onto a satellite CLI subcommand. The spawner pipes the
//! tool `arguments` JSON to the satellite's stdin (every satellite implements
//! the stdin-wins contract, so positional params work through stdin alone).

use serde_json::{json, Value};

/// Satellite binaries aggregated by secforge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Satellite {
    Neton,
    Firelin,
    Adonword,
    Howcueme,
    Memorypool,
}

impl Satellite {
    /// Binary name used on PATH.
    pub fn bin_name(self) -> &'static str {
        match self {
            Satellite::Neton => "neton",
            Satellite::Firelin => "firelin",
            Satellite::Adonword => "adonword",
            Satellite::Howcueme => "howcueme",
            Satellite::Memorypool => "memorypool",
        }
    }

    /// Env var that unlocks scan actions inside the satellite.
    pub fn permission_env(self) -> Option<(&'static str, &'static str)> {
        match self {
            Satellite::Neton => Some(("NETON_I_HAVE_PERMISSION", "yes")),
            Satellite::Firelin => Some(("FIRELIN_I_HAVE_PERMISSION", "yes")),
            _ => None,
        }
    }
}

/// One MCP tool backed by a satellite subcommand.
#[derive(Debug, Clone)]
pub struct ToolDef {
    /// MCP tool name, `<satellite>_<action>`.
    pub name: &'static str,
    /// English description surfaced in `tools/list`.
    pub description: &'static str,
    /// JSON Schema for the `arguments` object (piped to the satellite's stdin).
    pub input_schema: Value,
    pub satellite: Satellite,
    /// Subcommand argv, e.g. `&["baseline", "check"]`; the spawner appends `--json`.
    pub argv: &'static [&'static str],
    /// Requires the server to be started with `--yes-i-have-permission`.
    pub scan_gated: bool,
}

/// Build a JSON-Schema object from `(name, type, description, required)` tuples.
fn schema(fields: &[(&str, &str, &str, bool)]) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for (name, ty, desc, req) in fields {
        properties.insert(
            (*name).to_string(),
            json!({ "type": ty, "description": desc }),
        );
        if *req {
            required.push(json!(name));
        }
    }
    let mut schema = json!({ "type": "object", "properties": properties });
    if !required.is_empty() {
        schema["required"] = Value::Array(required);
    }
    schema
}

/// All tools secforge can expose, in stable satellite order.
pub fn tools() -> &'static [ToolDef] {
    static TOOLS: std::sync::OnceLock<Vec<ToolDef>> = std::sync::OnceLock::new();
    TOOLS.get_or_init(build_tools)
}

fn build_tools() -> Vec<ToolDef> {
    vec![
    // ── neton: network observation + authorized LAN scanning ─────────────
    ToolDef {
        name: "neton_info",
        description: "neton: host overview — hostname, OS, architecture, default outbound IP (zero packets sent).",
        input_schema: schema(&[]),
        satellite: Satellite::Neton,
        argv: &["info"],
        scan_gated: false,
    },
    ToolDef {
        name: "neton_interfaces",
        description: "neton: network interfaces — name, IPv4/IPv6 addresses, MAC, up/down status.",
        input_schema: schema(&[]),
        satellite: Satellite::Neton,
        argv: &["interfaces"],
        scan_gated: false,
    },
    ToolDef {
        name: "neton_ports",
        description: "neton: listening TCP sockets + bound UDP sockets with owning pid and process name.",
        input_schema: schema(&[("pid", "integer", "Only sockets owned by this pid", false)]),
        satellite: Satellite::Neton,
        argv: &["ports"],
        scan_gated: false,
    },
    ToolDef {
        name: "neton_dns",
        description: "neton: system resolver lookup returning IPv4/IPv6 arrays and elapsed ms.",
        input_schema: schema(&[("host", "string", "Hostname to resolve", true)]),
        satellite: Satellite::Neton,
        argv: &["dns"],
        scan_gated: false,
    },
    ToolDef {
        name: "neton_ping",
        description: "neton: TCP connect probe (no ICMP, no root) — success/latency/error.",
        input_schema: schema(&[
            ("host", "string", "Target host", true),
            ("port", "integer", "TCP port (default 443)", false),
            ("timeout_ms", "integer", "Connect timeout in ms (default 2000)", false),
        ]),
        satellite: Satellite::Neton,
        argv: &["ping"],
        scan_gated: false,
    },
    ToolDef {
        name: "neton_probe",
        description: "neton: concurrent TCP probing of many host:port targets (thread pool).",
        input_schema: schema(&[
            ("targets", "string", "Comma-separated host:port list, e.g. 'h1:80,h2:443'", true),
            ("concurrency", "integer", "Max concurrent probes (default 32)", false),
            ("timeout_ms", "integer", "Per-target timeout in ms (default 2000)", false),
        ]),
        satellite: Satellite::Neton,
        argv: &["probe"],
        scan_gated: false,
    },
    ToolDef {
        name: "neton_http",
        description: "neton: HTTP request summary — status, timing, headers (sensitive ones redacted), body preview.",
        input_schema: schema(&[
            ("url", "string", "URL to request", true),
            ("method", "string", "HTTP method (default GET)", false),
            ("timeout_ms", "integer", "Request timeout in ms (default 5000)", false),
            ("body_max", "integer", "Max body preview bytes (default 500)", false),
        ]),
        satellite: Satellite::Neton,
        argv: &["http"],
        scan_gated: false,
    },
    ToolDef {
        name: "neton_arp",
        description: "neton: system ARP/neighbor table (macOS/Linux/Windows) with MAC vendor lookup; read-only, no root.",
        input_schema: schema(&[]),
        satellite: Satellite::Neton,
        argv: &["arp"],
        scan_gated: false,
    },
    ToolDef {
        name: "neton_netscan",
        description: "neton [scan]: discover live devices in a subnet (TCP sweep + ARP correlation). Only scan networks you own or are authorized to test.",
        input_schema: schema(&[
            ("cidr", "string", "Subnet to scan, e.g. '192.168.1.0/24'", true),
            ("ports", "string", "Port spec: 'common', list '80,443' or range '1-1024' (default '22,80,443,445,3389,8080')", false),
            ("concurrency", "integer", "Max concurrent connects (default 128)", false),
            ("timeout_ms", "integer", "Connect timeout in ms (default 400)", false),
            ("no_rdns", "boolean", "Skip reverse-DNS lookups", false),
        ]),
        satellite: Satellite::Neton,
        argv: &["netscan"],
        scan_gated: true,
    },
    ToolDef {
        name: "neton_portscan",
        description: "neton [scan]: TCP connect port scan of one target. Only scan networks you own or are authorized to test.",
        input_schema: schema(&[
            ("target", "string", "Target IP or hostname", true),
            ("ports", "string", "Port spec: 'common' (40 top ports), list or range (default 'common')", false),
            ("concurrency", "integer", "Max concurrent connects (default 200)", false),
            ("timeout_ms", "integer", "Connect timeout in ms (default 800)", false),
        ]),
        satellite: Satellite::Neton,
        argv: &["portscan"],
        scan_gated: true,
    },
    ToolDef {
        name: "neton_device",
        description: "neton [scan]: analyze one LAN device — MAC vendor, reverse-DNS hostname, open ports, HTTP(S) fingerprint, device-type guess. Only scan networks you own or are authorized to test.",
        input_schema: schema(&[
            ("ip", "string", "Device IP address", true),
            ("ports", "string", "Port spec (default 'common')", false),
            ("timeout_ms", "integer", "Connect timeout in ms (default 1000)", false),
            ("http_max", "integer", "Max HTTP probes 0-4 (default 2)", false),
            ("no_rdns", "boolean", "Skip reverse-DNS lookups", false),
        ]),
        satellite: Satellite::Neton,
        argv: &["device"],
        scan_gated: true,
    },
    // ── firelin: authorized network assessment ───────────────────────────
    ToolDef {
        name: "firelin_portscan",
        description: "firelin [scan]: TCP connect port scan of one target or CIDR. Only scan networks you own or are authorized to test.",
        input_schema: schema(&[
            ("target", "string", "IP, hostname or CIDR", true),
            ("ports", "string", "Port spec (default '1-1024')", false),
            ("concurrency", "integer", "Max concurrent connects (default 200)", false),
            ("timeout_ms", "integer", "Connect timeout in ms (default 800)", false),
        ]),
        satellite: Satellite::Firelin,
        argv: &["portscan"],
        scan_gated: true,
    },
    ToolDef {
        name: "firelin_subdns",
        description: "firelin [scan]: DNS subdomain enumeration for one domain. Only test domains you own or are authorized to test.",
        input_schema: schema(&[
            ("domain", "string", "Domain to enumerate", true),
            ("wordlist", "string", "Subdomain wordlist (default builtin)", false),
            ("resolver", "string", "DNS resolver host:port (default '8.8.8.8:53')", false),
            ("concurrency", "integer", "Max concurrent queries (default 50)", false),
        ]),
        satellite: Satellite::Firelin,
        argv: &["subdns"],
        scan_gated: true,
    },
    ToolDef {
        name: "firelin_dirscan",
        description: "firelin [scan]: HTTP directory/path enumeration of one web server. Only scan servers you own or are authorized to test.",
        input_schema: schema(&[
            ("url", "string", "Base URL to scan", true),
            ("wordlist", "string", "Path wordlist (default builtin)", false),
            ("concurrency", "integer", "Max concurrent requests (default 20)", false),
            ("timeout_ms", "integer", "Request timeout in ms (default 3000)", false),
            ("follow_redirects", "boolean", "Follow 3xx redirects (default false)", false),
        ]),
        satellite: Satellite::Firelin,
        argv: &["dirscan"],
        scan_gated: true,
    },
    ToolDef {
        name: "firelin_fingerprint",
        description: "firelin: HTTP fingerprint of one URL — status, Server, X-Powered-By, page title (read-only GET).",
        input_schema: schema(&[
            ("url", "string", "URL to fingerprint", true),
            ("timeout_ms", "integer", "Request timeout in ms (default 5000)", false),
        ]),
        satellite: Satellite::Firelin,
        argv: &["fingerprint"],
        scan_gated: false,
    },
    ToolDef {
        name: "firelin_cidr",
        description: "firelin: expand a CIDR into its address list (offline, no packets sent).",
        input_schema: schema(&[("cidr", "string", "CIDR, e.g. '192.168.1.0/24'", true)]),
        satellite: Satellite::Firelin,
        argv: &["cidr"],
        scan_gated: false,
    },
    // ── adonword: active-defense sentinel ────────────────────────────────
    ToolDef {
        name: "adonword_scan",
        description: "adonword: one full inspection — file-tree vs baseline diffs, blacklisted processes, unallowlisted listening ports.",
        input_schema: schema(&[]),
        satellite: Satellite::Adonword,
        argv: &["scan"],
        scan_gated: false,
    },
    ToolDef {
        name: "adonword_baseline_check",
        description: "adonword: compare the file tree against the sha256 baseline (added/removed/modified).",
        input_schema: schema(&[]),
        satellite: Satellite::Adonword,
        argv: &["baseline", "check"],
        scan_gated: false,
    },
    ToolDef {
        name: "adonword_report",
        description: "adonword: last persisted scan result (findings + summary).",
        input_schema: schema(&[]),
        satellite: Satellite::Adonword,
        argv: &["report"],
        scan_gated: false,
    },
    // ── howcueme: conditional self-wakeup daemon ─────────────────────────
    ToolDef {
        name: "howcueme_list",
        description: "howcueme: all wake-up rules with their last trigger times (from state.json).",
        input_schema: schema(&[]),
        satellite: Satellite::Howcueme,
        argv: &["list"],
        scan_gated: false,
    },
    ToolDef {
        name: "howcueme_validate",
        description: "howcueme: validate the rules.toml file — rules count + errors.",
        input_schema: schema(&[]),
        satellite: Satellite::Howcueme,
        argv: &["validate"],
        scan_gated: false,
    },
    ToolDef {
        name: "howcueme_fire",
        description: "howcueme: force-trigger one wake-up rule now, bypassing its condition and cooldown (test helper).",
        input_schema: schema(&[("rule", "string", "Rule name to trigger", true)]),
        satellite: Satellite::Howcueme,
        argv: &["fire"],
        scan_gated: false,
    },
    ToolDef {
        name: "howcueme_run_once",
        description: "howcueme: evaluate all wake-up rules for one round — any rule whose condition is met fires its action.",
        input_schema: schema(&[]),
        satellite: Satellite::Howcueme,
        argv: &["run", "--once"],
        scan_gated: false,
    },
    // ── memorypool: shared agent memory ──────────────────────────────────
    ToolDef {
        name: "memorypool_add",
        description: "memorypool: store one memory (text + tags + importance) in the shared pool.",
        input_schema: schema(&[
            ("text", "string", "Memory content", true),
            ("tags", "array", "Free-form labels", false),
            ("importance", "number", "Importance 0.0-1.0 (default 0.5)", false),
            ("source", "string", "Origin (default 'cli'; agents should use 'bit')", false),
        ]),
        satellite: Satellite::Memorypool,
        argv: &["add"],
        scan_gated: false,
    },
    ToolDef {
        name: "memorypool_search",
        description: "memorypool: ranked keyword search — term hits, tag hits, importance weighting, 30-day half-life time decay.",
        input_schema: schema(&[
            ("query", "string", "Search keywords", true),
            ("tag", "string", "Tag filter", false),
            ("limit", "integer", "Max results", false),
        ]),
        satellite: Satellite::Memorypool,
        argv: &["search"],
        scan_gated: false,
    },
    ToolDef {
        name: "memorypool_list",
        description: "memorypool: list memories, newest first, optional tag filter.",
        input_schema: schema(&[
            ("tag", "string", "Tag filter", false),
            ("limit", "integer", "Max results", false),
        ]),
        satellite: Satellite::Memorypool,
        argv: &["list"],
        scan_gated: false,
    },
    ToolDef {
        name: "memorypool_get",
        description: "memorypool: fetch one memory by id.",
        input_schema: schema(&[("id", "string", "Memory id", true)]),
        satellite: Satellite::Memorypool,
        argv: &["get"],
        scan_gated: false,
    },
    ToolDef {
        name: "memorypool_delete",
        description: "memorypool: delete one memory by id.",
        input_schema: schema(&[("id", "string", "Memory id", true)]),
        satellite: Satellite::Memorypool,
        argv: &["delete"],
        scan_gated: false,
    },
    ToolDef {
        name: "memorypool_stats",
        description: "memorypool: pool statistics — count, tag histogram, time range.",
        input_schema: schema(&[]),
        satellite: Satellite::Memorypool,
        argv: &["stats"],
        scan_gated: false,
    },
    ]
}

/// All five satellites in registry order.
pub const SATELLITES: &[Satellite] = &[
    Satellite::Neton,
    Satellite::Firelin,
    Satellite::Adonword,
    Satellite::Howcueme,
    Satellite::Memorypool,
];
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_shape() {
        let all = tools();
        assert_eq!(all.len(), 29, "29 tools registered");
        let mut names: Vec<&str> = all.iter().map(|t| t.name).collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total, "tool names must be unique");

        for tool in all {
            assert!(
                tool.name.contains('_'),
                "{} must be satellite_action",
                tool.name
            );
            assert!(!tool.description.is_empty());
            assert_eq!(tool.input_schema["type"], "object");
            let gated = matches!(
                tool.name,
                "neton_netscan"
                    | "neton_portscan"
                    | "neton_device"
                    | "firelin_portscan"
                    | "firelin_subdns"
                    | "firelin_dirscan"
            );
            assert_eq!(tool.scan_gated, gated, "{} scan gating mismatch", tool.name);
        }
    }

    #[test]
    fn satellite_permission_envs() {
        assert_eq!(
            Satellite::Neton.permission_env().map(|e| e.0),
            Some("NETON_I_HAVE_PERMISSION")
        );
        assert_eq!(
            Satellite::Firelin.permission_env().map(|e| e.0),
            Some("FIRELIN_I_HAVE_PERMISSION")
        );
        assert!(Satellite::Adonword.permission_env().is_none());
        assert_eq!(Satellite::Neton.bin_name(), "neton");
    }
}
