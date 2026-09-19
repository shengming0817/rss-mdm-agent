# ACP 接入

`createAccessService({ host, sessionOptions })` 消费公共 `HostPort`。组合根通过 `service.connect(stream, caller)` 注入可信 caller；请求、卡片、模型文本不能提供身份。每个连接/会话只有一个 Host 订阅，转换为标准 ACP 更新及已协商产品更新。

标准 `initialize`、`session/new`、`session/list`、`session/load`、`session/prompt`、`session/cancel` 和 `session/resume` 使用官方 SDK1.4.0。prompt 等待确定终态；`failed` 返回错误，其他结局映射到 ACP stopReason。resource_link 仅转为明确标记的文本引用，不读取 URI、文件或客户端路径。MCP server 注入与多模态输入未提供能力。load 恢复展示历史，resume 调用 Host 的原生续接能力，两者分别失败或成功。

产品客户端通过 `clientCapabilities._meta[extension.capability]` 提交 ai-contract 的 `Negotiation`，服务返回所选能力。`_rss-mdm-agent/submit` 返回接纳 receipt；snapshot/list/attach/detach/action/resume 使用同一个产品 schema。未经协商不能调用扩展。A2UI 仅接纳固定版本、catalog 和完整有界恢复消息，失效内容触发重新同步，不作为成功更新发布。

`requestPermission(caller, upstreamRequest, signal)` 仅向该 caller 已附着的会话连接递送标准 ACP 权限问题/选项。它消费组合根持有的实时回调期限，首个有效回答生效，取消后不恢复回调；它没有权限数据库，也不签发 Rust 执行批准。普通 `Interaction.category=question` 保持独立。

SDK `Stream` 是唯一 transport 接缝；直接提供 `ndJsonStream`。本地/Tauri channel 适配由 ai-client 的 `channelStream` 提供，实际桌面组合根归 #2413。连接关闭只释放订阅和请求，不关闭共享 Host、不取消已接纳命令。HTTP/relay 没有生产实现；独立验收中的 loopback HTTP 仅传送 SDK 测试消息。

运行 `pnpm test:ai-access`、`pnpm check:ai-access-consumer`。版本、边界及消费方法见[开发说明](../../docs/guides/ai-access-development.md)。
