# SECFORGE 测试说明

- 测试完成：是（2026-10-04）
- 测试日期：2026-10-04
- 测试内容：单元测试 2（工具注册表/权限环境）+ CLI tests/cli.rs(6) + MCP tests/mcp.rs(6)；注入：shell 元字符参数走 stdin JSON 不被执行（无文件副作用）、非对象/垃圾参数被干净处理不 panic；本仓库无插件/钩子机制。
- 运行命令：`cargo test`（mcp 用例需 rustc 在 PATH 现场编译假卫星）
- 测试框架：Rust `#[cfg(test)]` + 进程内 axum 路由端到端
- 模型：豆包（Doubao）生成

# 测试说明（SECFORGE）

SECFORGE 是一个 MCP 聚合网关：把多个安全工具卫星二进制（neton/firelin/...）聚合为
MCP 工具。测试分三层：`src/` 单元测试、`tests/cli.rs`（真实二进制 CLI）、
`tests/mcp.rs`（进程内起 MCP 路由 + 现场用 rustc 编译的假卫星）。

## 怎么跑

```powershell
cargo test                 # 全部
cargo test --test cli      # CLI / tools / serve
cargo test --test mcp      # MCP JSON-RPC 端到端（含注入测试）
```

> `tests/mcp.rs` 会用 `rustc` 现场编译一个假卫星（fake satellite），需要工具链在 PATH。
> 测试全程在 127.0.0.1 随机端口、临时目录里完成，**不需要外网、不扫描真实主机**。

## 预期结果

全部通过，0 失败：

- `src/lib.rs` 单元：2（注册表结构、权限环境）。
- `tests/cli.rs`：6（--help/--version/tools/无卫星/serve 健康与 MCP 握手）。
- `tests/mcp.rs`：6（握手→tools/list→tools/call、扫描授权门、卫星失败→isError、
  协议错误码、**注入测试 ×2**）。

## 测了什么

- **协议**：initialize 握手、session id、notifications、ping、tools/list、
  tools/call、未知工具 -32602、未知方法 -32601、畸形 JSON -32700、版本回退。
- **授权门**：未加 `--yes-i-have-permission` 时扫描工具返回 isError；解锁后带
  权限环境变量给子进程。
- **注入（`tests/mcp.rs`）**：SECFORGE 把工具参数以 **stdin JSON** 传给卫星，
  **不走 shell**。断言：
  - `hostile_argument_values_are_passed_as_literal_stdin_data_not_shelled`：参数值里
    的 `"; touch x; $(whoami) #"` 载荷作为不透明字符串走 stdin 传入，**不执行**
    （断言没有产生任何文件）。
  - `non_object_and_garbage_arguments_are_handled_without_panic`：arguments 传成
    裸字符串/深层嵌套数组，被干净地处理，路由器不 panic。

## 备注

- 本仓库**没有插件/钩子/事件回调机制**（纯 MCP 聚合网关，工具注册表是静态的），
  故不涉及钩子生命周期测试。
- 路径穿越/XSS 不适用：网关不提供静态文件服务、不渲染 HTML（一律返回 JSON）。
