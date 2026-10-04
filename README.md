# SECFORGE

Security MCP server for AI agents — aggregates the 5 BITECO satellite tools
(neton / firelin / adonword / howcueme / memorypool) as **29 MCP tools** served over
JSON-RPC 2.0 on Streamable HTTP.

[![Release](https://img.shields.io/github/v/release/yxpil/SECFORGE?style=flat-square)](https://github.com/yxpil/SECFORGE/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/yxpil/SECFORGE/total?style=flat-square)](https://github.com/yxpil/SECFORGE/releases)
[![License](https://img.shields.io/badge/license-Apache--2.0-black?style=flat-square)](./LICENSE)
[![CI](https://github.com/yxpil/SECFORGE/actions/workflows/ci.yml/badge.svg?style=flat-square)](https://github.com/yxpil/SECFORGE/actions/workflows/ci.yml)
[![Platform](https://img.shields.io/badge/platform-windows%20%7C%20macos%20%7C%20linux-black?style=flat-square)](https://github.com/yxpil/SECFORGE)

## About

SECFORGE is a single MCP (Model Context Protocol) server that fronts the whole BITECO
security toolchain. Point an MCP client — [BIT](https://github.com/yxpil/bit), the desktop
AI agent hub, or any other Streamable-HTTP MCP client — at one URL and the agent gets:

- **neton** (11 tools) — local network observation + authorized LAN scanning
- **firelin** (5 tools) — authorized network assessment (port scan, subdomain DNS, dir scan, fingerprint, CIDR)
- **adonword** (3 tools) — active-defense sentinel (baseline diffs, blacklisted processes, port allowlist)
- **howcueme** (4 tools) — conditional self-wakeup daemon (cron-like rules for agents)
- **memorypool** (6 tools) — persistent scored memory pool for agents

Each MCP tool maps 1:1 onto a satellite CLI subcommand. SECFORGE spawns the satellite
binary, pipes the tool arguments as a JSON object into its stdin, and returns the JSON from
stdout as the tool result. Satellites that are not installed stay hidden — the tool list
shrinks instead of erroring. Nothing else is required: no Python, no Node, one static binary.

Protocol compatibility was verified against BIT's MCP client (initialize handshake,
`Mcp-Session-Id`, `tools/list`, `tools/call`), and it speaks `2024-11-05`,
`2025-03-26` and `2025-06-18` protocol versions.

### Scan authorization (important)

Six tools touch *other* hosts (`neton_netscan`, `neton_portscan`, `neton_device`,
`firelin_portscan`, `firelin_subdns`, `firelin_dirscan`). They are locked until the
**server operator** acknowledges authorization:

```bash
secforge serve --yes-i-have-permission
# or: SECFORGE_I_HAVE_PERMISSION=yes secforge serve
```

- Without the acknowledgement, scan tools are **not listed at all** and a direct call
  returns an `isError` result explaining the flag.
- With the acknowledgement they are listed, and the per-satellite permission environment
  variable (e.g. `NETON_I_HAVE_PERMISSION=yes`) is forwarded to the satellite so its own
  gate passes too.
- Only scan networks you own or are explicitly authorized to test.

## Install

Download a prebuilt binary from [Releases](https://github.com/yxpil/SECFORGE/releases/latest):

| Platform | File |
| --- | --- |
| Linux x86_64 | `secforge-v0.1.0-x86_64-unknown-linux-gnu.tar.gz` |
| macOS Apple Silicon | `secforge-v0.1.0-aarch64-apple-darwin.tar.gz` |
| Windows x86_64 | `secforge-v0.1.0-x86_64-pc-windows-msvc.zip` |

```bash
tar xzf secforge-v0.1.0-aarch64-apple-darwin.tar.gz
sudo mv secforge /usr/local/bin/   # or anywhere on PATH
secforge --version
```

Build from source:

```bash
cargo install --git https://github.com/yxpil/SECFORGE
```

Install the satellites you want the same way (see their repos: [Neton](https://github.com/yxpil/Neton),
[Firelin](https://github.com/yxpil/Firelin), [ADONWORD](https://github.com/yxpil/ADONWORD),
[HOWCUEME](https://github.com/yxpil/HOWCUEME), [MemoryPool](https://github.com/yxpil/MemoryPool)),
or point SECFORGE at them explicitly with `--neton <path>`, `--firelin <path>`, etc.

## Quick start

```bash
# 1. See what would be exposed (JSON, no server started)
secforge tools

# 2. Start the MCP server (default 127.0.0.1:8756)
secforge serve

# 3. With scan tools unlocked and an explicit satellite path
secforge serve --yes-i-have-permission --neton /usr/local/bin/neton
```

The endpoint answers on `http://127.0.0.1:8756/` and `/mcp`. Health check:

```bash
curl http://127.0.0.1:8756/health
# {"ok":true,"satellites":["neton"],"scan_authorized":false,"service":"secforge","tools":11,"version":"0.1.0"}
```

## MCP protocol

JSON-RPC 2.0 over Streamable HTTP. Responses are `application/json`.

```bash
# 1. initialize
curl -s -D headers.txt -X POST http://127.0.0.1:8756/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"me","version":"0"}}}'
# serverInfo: {"name":"secforge","version":"0.1.0"}; capture Mcp-Session-Id from the headers

# 2. notifications/initialized (same session id)
# 3. tools/list — array of {name, description, inputSchema}
# 4. tools/call
curl -s -X POST http://127.0.0.1:8756/mcp -H "Mcp-Session-Id: $SID" \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"neton_info","arguments":{}}}'
# {"content":[{"text":"{\"hostname\":\"…\",\"os\":\"…\",\"arch\":\"…\",\"outbound_ip\":\"…\"}","type":"text"}],"isError":false}
```

Tool results are returned as `content: [{type:"text", ...}]`. Satellite failures and
gate violations come back as `isError: true` results (not transport errors) so the agent
can read the reason and self-correct.

### Tool inventory

| Satellite | Tools | Scan-gated |
| --- | --- | --- |
| neton | `info` `interfaces` `ports` `dns` `ping` `probe` `http` `arp` · `netscan` `portscan` `device` | last 3 |
| firelin | `fingerprint` `cidr` · `portscan` `subdns` `dirscan` | last 3 |
| adonword | `scan` `baseline_check` `report` | — |
| howcueme | `list` `validate` `fire` `run_once` | — |
| memorypool | `add` `search` `list` `get` `delete` `stats` | — |

`secforge tools` prints the exact live inventory (after satellite detection) as JSON.

## BIT integration

In BIT (desktop AI agent hub), open the MCP servers page and add `http://127.0.0.1:8756`
— or let BIT's discovery find it on that port. After the handshake BIT lists every
exposed satellite tool and the agent can call them like any native tool. Any MCP client
speaking Streamable HTTP works the same way.

### Typical agent exchange

> **user:** what's listening on my machine and is anything odd in the last scan?
> **agent:** calls `neton_ports`, then `adonword_report` — both zero-packet, read-only.
> **user:** now find every device on my home subnet.
> **agent:** calls `neton_netscan` — if the server was started without
> `--yes-i-have-permission` the call returns the authorization hint instead of scanning.

## API

### CLI

```
secforge [--yes-i-have-permission] [--neton PATH] [--firelin PATH] [--adonword PATH]
         [--howcueme PATH] [--memorypool PATH] <command>

serve [--host 127.0.0.1] [--port 8756]   run the MCP server (Streamable HTTP)
tools                                    print the exposed tools as JSON
```

- Satellite binaries are resolved from `PATH`, or from the explicit `--<name> PATH` flag.
- Logs go to stderr; the endpoint and detection report appear at startup.

### HTTP

| Route | Method | Purpose |
| --- | --- | --- |
| `/health` | GET | liveness + exposed tool count + satellites + scan state |
| `/` and `/mcp` | POST | JSON-RPC 2.0 (`initialize`, `notifications/initialized`, `tools/list`, `tools/call`, `ping`) |
| `/` and `/mcp` | DELETE | session teardown (per MCP spec) |

Session handling follows the MCP spec: `initialize` returns an `Mcp-Session-Id` header
(`mcp-<hex>`); subsequent requests should carry it. Requests with a stale/unknown session
get JSON-RPC error `-32001`. Unknown methods get `-32601`; unknown tools `-32602`.

## Notes

- Bind address defaults to `127.0.0.1` (loopback only) — expose it deliberately.
- Every tools/call spawns a fresh satellite process with a 30 s timeout; there is no shared
  state between calls beyond the satellites' own storage.
- Scan tools are gated at the *server* level (not listed + not callable) and at the
  *satellite* level (its own permission env var is forwarded) — two independent locks.

## 中文

### 简介

SECFORGE 是面向 AI 代理的网络安全 MCP 服务器：把 BITECO 生态的 5 个卫星工具
（neton / firelin / adonword / howcueme / memorypool）聚合为 **29 个 MCP 工具**，
通过 JSON-RPC 2.0（Streamable HTTP）暴露。BIT 或任何 MCP 客户端只需配置一个 URL，
代理即可获得网络观测、授权扫描、主机防御、定时唤醒与持久记忆的完整能力。

每个 MCP 工具 1:1 映射一个卫星 CLI 子命令：SECFORGE 启动卫星进程，把工具参数作为
JSON 对象写入其 stdin，并把 stdout 的 JSON 作为工具结果返回。未安装的卫星自动隐藏，
工具列表收缩而非报错；单二进制、零运行时依赖。

协议与 BIT 的 MCP 客户端对齐验证（initialize 握手、`Mcp-Session-Id`、`tools/list`、
`tools/call`），兼容 `2024-11-05`、`2025-03-26`、`2025-06-18` 三个协议版本。

### 扫描授权（重要）

六个工具会触及其它主机（`neton_netscan`、`neton_portscan`、`neton_device`、
`firelin_portscan`、`firelin_subdns`、`firelin_dirscan`）。默认锁定，需要服务器操作者
显式授权：

```bash
secforge serve --yes-i-have-permission
# 或: SECFORGE_I_HAVE_PERMISSION=yes secforge serve
```

- 未授权时扫描工具**完全不出现在工具列表**中，直接调用返回 `isError` 结果并提示授权方式。
- 授权后工具可见，同时向卫星转发各自的授权环境变量（如 `NETON_I_HAVE_PERMISSION=yes`），
  卫星自身的门禁也一并放行。
- 只扫描你拥有或被明确授权测试的网络。

### 安装

从 [Releases](https://github.com/yxpil/SECFORGE/releases/latest) 下载对应平台的预编译二进制：

| 平台 | 文件 |
| --- | --- |
| Linux x86_64 | `secforge-v0.1.0-x86_64-unknown-linux-gnu.tar.gz` |
| macOS Apple Silicon | `secforge-v0.1.0-aarch64-apple-darwin.tar.gz` |
| Windows x86_64 | `secforge-v0.1.0-x86_64-pc-windows-msvc.zip` |

```bash
tar xzf secforge-v0.1.0-aarch64-apple-darwin.tar.gz
sudo mv secforge /usr/local/bin/
secforge --version
```

源码安装：

```bash
cargo install --git https://github.com/yxpil/SECFORGE
```

卫星工具用同样方式安装（见各自仓库），或用 `--neton <路径>`、`--firelin <路径>` 等
显式指定位置。

### 快速上手

```bash
# 1. 查看将暴露的工具（JSON，不启动服务器）
secforge tools

# 2. 启动 MCP 服务器（默认 127.0.0.1:8756）
secforge serve

# 3. 解锁扫描工具并显式指定卫星路径
secforge serve --yes-i-have-permission --neton /usr/local/bin/neton
```

端点在 `http://127.0.0.1:8756/` 与 `/mcp`。健康检查：

```bash
curl http://127.0.0.1:8756/health
# {"ok":true,"satellites":["neton"],"scan_authorized":false,"service":"secforge","tools":11,"version":"0.1.0"}
```

### 工具清单

| 卫星 | 工具 | 扫描门控 |
| --- | --- | --- |
| neton | `info` `interfaces` `ports` `dns` `ping` `probe` `http` `arp` · `netscan` `portscan` `device` | 后 3 个 |
| firelin | `fingerprint` `cidr` · `portscan` `subdns` `dirscan` | 后 3 个 |
| adonword | `scan` `baseline_check` `report` | — |
| howcueme | `list` `validate` `fire` `run_once` | — |
| memorypool | `add` `search` `list` `get` `delete` `stats` | — |

`secforge tools` 输出卫星探测后的真实清单（JSON）。

### BIT 集成

在 BIT 的 MCP 服务器页面添加 `http://127.0.0.1:8756`（或让 BIT 自动发现该端口）。
握手完成后 BIT 会列出所有可见工具，代理即可像调用原生工具一样调用它们。任何支持
Streamable HTTP 的 MCP 客户端接入方式相同。

### 安全与合规

- 默认只绑定 `127.0.0.1`（仅本机回环），如需对外暴露请自行权衡。
- 每次 `tools/call` 都启动一个全新的卫星进程，30 秒超时，调用之间无共享状态。
- 扫描工具在**服务器层**（不列出 + 不可调用）和**卫星层**（转发其自身授权环境变量）
  双重锁定，两道门独立生效。
- 仅对你拥有或获得书面授权的网络执行扫描。

---

<div align="center">

<a href="https://github.com/yxpil/SECFORGE">
  <img width="100%" src="https://alittlecatgirlpanel.yxp.hk/card?repo=yxpil/SECFORGE" alt="gh-card · yxpil/SECFORGE" />
</a>

<sub>Powered by <a href="https://alittlecatgirlpanel.yxp.hk"><b>gh-card</b></a> · 粉色手写体 README 仓库名片</sub>

</div>
