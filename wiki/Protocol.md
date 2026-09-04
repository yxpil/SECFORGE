# SECFORGE MCP Protocol / SECFORGE MCP 协议

SECFORGE implements MCP (Model Context Protocol) over **Streamable HTTP**: JSON-RPC 2.0 requests POSTed to `/` or `/mcp`, responses as `application/json`. Session handling follows the MCP spec via the `Mcp-Session-Id` header. Protocol versions negotiated at `initialize`: `2024-11-05`, `2025-03-26`, `2025-06-18`.

SECFORGE 通过 **Streamable HTTP** 实现 MCP（Model Context Protocol）：JSON-RPC 2.0 请求 POST 到 `/` 或 `/mcp`，响应为 `application/json`。会话由 `Mcp-Session-Id` 头管理，遵循 MCP 规范。`initialize` 时协商协议版本：`2024-11-05`、`2025-03-26`、`2025-06-18`。

## Endpoints / 端点

| Endpoint / 端点 | Method / 方法 | Auth / 认证 | Description / 说明 |
| --- | --- | --- | --- |
| `/health` | GET | none / 无 | `{"ok":true,"satellites":["neton"],"scan_authorized":false,"service":"secforge","tools":11,"version":"0.1.0"}` |
| `/` and `/mcp` | POST | none (loopback bind / 仅回环绑定) | JSON-RPC 2.0 entry / JSON-RPC 2.0 入口 |
| `/` and `/mcp` | DELETE | Mcp-Session-Id | session teardown / 会话销毁 |

## Handshake / 握手

```json
// 1. request
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{
  "protocolVersion":"2025-03-26","capabilities":{},
  "clientInfo":{"name":"bit","version":"1.0"}}}

// 2. response — echo of the negotiated version + Mcp-Session-Id: mcp-<hex> header
{"jsonrpc":"2.0","id":1,"result":{
  "protocolVersion":"2025-03-26",
  "capabilities":{"tools":{"listChanged":false}},
  "serverInfo":{"name":"secforge","version":"0.1.0"}}}

// 3. notification (same session id)
{"jsonrpc":"2.0","method":"notifications/initialized"}
```

Subsequent requests must carry `Mcp-Session-Id: mcp-<hex>`. A stale/unknown session gets error `-32001`. / 后续请求须携带该会话头；过期或未知会话返回 `-32001`。

## Methods / 方法

| Method / 方法 | Params / 参数 | Result / 结果 |
| --- | --- | --- |
| `initialize` | `protocolVersion`, `capabilities`, `clientInfo` | `protocolVersion`, `capabilities`, `serverInfo` |
| `notifications/initialized` | — | `202 Accepted`, empty body / 空体 |
| `ping` | — | `{}` |
| `tools/list` | — | `{"tools":[{name, description, inputSchema}, …]}` |
| `tools/call` | `name`, `arguments` | `{"content":[{"type":"text","text":"…"}],"isError":false}` |

## Error codes / 错误码

| Code / 码 | Meaning / 含义 |
| --- | --- |
| `-32700` | body is not valid JSON / 请求体非法 JSON |
| `-32600` | missing `method` / 缺少 method |
| `-32601` | unknown JSON-RPC method / 未知方法 |
| `-32602` | unknown tool or bad `tools/call` params / 未知工具或参数错误 |
| `-32001` | stale/unknown Mcp-Session-Id / 会话失效 |

## Scan gate in the protocol / 协议中的扫描门控

When the server runs without `--yes-i-have-permission` (or `SECFORGE_I_HAVE_PERMISSION=yes`):

- the 6 scan tools are **absent from `tools/list`**; / 6 个扫描工具**不出现在 tools/list**；
- a direct `tools/call` for them returns an `isError: true` result whose text explains the flag — a *result*, not a transport error, so the agent can read and self-correct; / 直接调用返回 `isError: true` 的结果文本，提示授权方式——是结果而非传输错误，智能体可读取并自我纠正；
- there is no per-request authorization — the gate binds at server startup. / 不支持按请求授权——门控在服务器启动时绑定。

With the flag set, the tools appear and the per-satellite permission env (e.g. `NETON_I_HAVE_PERMISSION=yes`) is forwarded to the satellite process so its own gate passes. / 授权后工具出现，并向卫星进程转发其自身授权环境变量。

## Tool result mapping / 工具结果映射

Each MCP tool maps 1:1 to a satellite subcommand. `tools/call` spawns the satellite, pipes `arguments` as a JSON object into its stdin, reads one JSON value from stdout, and wraps it:

每个 MCP 工具 1:1 映射一个卫星子命令。`tools/call` 启动卫星进程，把 `arguments` 作为 JSON 对象写入其 stdin，从 stdout 读取一个 JSON 值并包装：

```json
{"content":[{"type":"text","text":"<satellite stdout JSON>"}],"isError":false}
```

Satellite non-zero exit or stderr output becomes `isError: true` with the reason in `text`. Each call has a 30 s timeout (`kill_on_drop`). / 卫星非零退出或 stderr 输出变为 `isError: true` 并附原因。每次调用 30 秒超时（`kill_on_drop`）。

## Example session / 示例会话

```bash
SID=$(curl -s -D - -o /dev/null -X POST http://127.0.0.1:8756/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"me","version":"0"}}}' \
  | grep -i mcp-session-id | awk '{print $2}' | tr -d '\r')

curl -s -X POST http://127.0.0.1:8756/mcp -H "Mcp-Session-Id: $SID" \
  -d '{"jsonrpc":"2.0","method":"notifications/initialized"}'

curl -s -X POST http://127.0.0.1:8756/mcp -H "Mcp-Session-Id: $SID" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"neton_info","arguments":{}}}'
# {"id":2,"jsonrpc":"2.0","result":{"content":[{"text":"{\"arch\":\"aarch64\",\"hostname\":\"…\",\"os\":\"…\",\"outbound_ip\":\"…\"}","type":"text"}],"isError":false}}
```
