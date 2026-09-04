# SECFORGE Wiki / SECFORGE 维基

**SECFORGE** — Security MCP server for AI agents around the [BIT](https://github.com/yxpil/bit) ecosystem: it aggregates the 5 BITECO satellites (neton / firelin / adonword / howcueme / memorypool) as **29 MCP tools** served over JSON-RPC 2.0 on Streamable HTTP. One URL gives an agent network observation, authorized scanning, host defense, conditional wakeup and persistent memory.

**SECFORGE** —— 面向 [BIT](https://github.com/yxpil/bit) 生态 AI 智能体的网络安全 MCP 服务器：将 5 个 BITECO 卫星工具（neton / firelin / adonword / howcueme / memorypool）聚合为 **29 个 MCP 工具**，通过 JSON-RPC 2.0（Streamable HTTP）暴露。只需一个 URL，智能体即可获得网络观测、授权扫描、主机防御、条件唤醒与持久记忆的完整能力。

- Repo / 仓库: <https://github.com/yxpil/SECFORGE>
- Releases / 发行版: <https://github.com/yxpil/SECFORGE/releases>
- Binary name / 二进制名: `secforge`
- Default port / 默认端口: `8756`
- Endpoint / 端点: `http://127.0.0.1:8756/` and `/mcp`

> **⚠️ The 6 scan tools only run when the server is started with explicit authorization. / 6 个扫描工具只有在服务器显式授权启动后才会运行。**

---

## Satellites / 卫星工具

| Satellite | Tools | What it gives the agent / 提供能力 | Scan-gated / 扫描门控 |
| --- | --- | --- | --- |
| [neton](https://github.com/yxpil/Neton) | 11 | network observation: info, interfaces, ports, dns, ping, probe, http, arp · scans: netscan, portscan, device / 网络观测 + 授权扫描 | last 3 / 后 3 个 |
| [firelin](https://github.com/yxpil/Firelin) | 5 | assessment: fingerprint, cidr · scans: portscan, subdns, dirscan / 授权评估 | last 3 / 后 3 个 |
| [adonword](https://github.com/yxpil/ADONWORD) | 3 | active defense: scan, baseline_check, report / 主动防御 | — |
| [howcueme](https://github.com/yxpil/HOWCUEME) | 4 | conditional wakeup: list, validate, fire, run_once / 条件唤醒 | — |
| [memorypool](https://github.com/yxpil/MemoryPool) | 6 | persistent memory: add, search, list, get, delete, stats / 持久记忆 | — |

Not installed a satellite? Its tools simply stay hidden — the tool list shrinks, nothing errors. / 未安装的卫星自动隐藏，工具列表收缩而非报错。

## Install / 安装

Grab a per-platform binary from the [latest release](https://github.com/yxpil/SECFORGE/releases/latest), or:

从 [最新 Release](https://github.com/yxpil/SECFORGE/releases/latest) 下载对应平台二进制，或：

```bash
cargo install --git https://github.com/yxpil/SECFORGE
```

Install the satellites you want the same way (their own repos), or point SECFORGE at them explicitly. / 卫星用同样方式安装（见各自仓库），或显式指定路径。

## Scan authorization / 扫描授权

```bash
secforge serve --yes-i-have-permission          # unlock scan tools / 解锁扫描工具
SECFORGE_I_HAVE_PERMISSION=yes secforge serve   # same via env / 环境变量等价
```

- Without the flag: scan tools are **not listed** and a direct call returns an `isError` hint. / 未授权：扫描工具不列出，直接调用返回授权提示。
- With the flag: tools are listed and the per-satellite permission env (e.g. `NETON_I_HAVE_PERMISSION=yes`) is forwarded, so the satellite's own gate passes too. / 授权后工具可见，并向卫星转发其自身授权环境变量。
- Only scan networks you own or are authorized to test. / 只扫描你拥有或获授权的网络。

## Quick start / 快速上手

```bash
secforge tools                                  # live tool inventory as JSON / 真实工具清单
secforge serve                                  # MCP server on 127.0.0.1:8756
curl http://127.0.0.1:8756/health
# {"ok":true,"satellites":["neton"],"scan_authorized":false,"service":"secforge","tools":11,"version":"0.1.0"}
```

## BIT integration / BIT 集成

In BIT's MCP servers page add `http://127.0.0.1:8756`, or let discovery find it on that port. Any Streamable-HTTP MCP client works the same way. / 在 BIT 的 MCP 服务器页面添加该 URL，或让 BIT 自动发现；任何支持 Streamable HTTP 的 MCP 客户端接入方式相同。

## FAQ

**Q: Do I need Python/Node for the MCP server? / 需要 Python 或 Node 吗？**
No. One static Rust binary; each tool call spawns the satellite CLI. / 不需要。单一 Rust 二进制；每次调用启动卫星 CLI。

**Q: Can SECFORGE run without any satellite installed? / 一个卫星都没装能跑吗？**
Yes — it serves 0 tools with a healthy `/health`. Install satellites and the list grows. / 可以——`/health` 正常但 0 个工具；装上卫星后列表自动增长。

**Q: Where do satellites have to be? / 卫星必须装在哪？**
On `PATH`, or pass `--neton/--firelin/--adonword/--howcueme/--memorypool <path>`. / 在 PATH 中，或用对应参数显式指定路径。

**Q: How do two lock levels work? / 两道门如何配合？**
Server level: gated tools hidden + calls refused. Satellite level: SECFORGE forwards the satellite's own permission env var so its gate passes. Both must agree. / 服务器层隐藏并拒绝调用；卫星层转发授权环境变量。两道门独立生效。
