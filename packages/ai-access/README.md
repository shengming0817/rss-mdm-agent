# ACP 接入

`createAccessService({ host, sessionOptions })` 消费公共 `HostPort`。组合根通过 `service.connect(stream, caller)` 注入可信 caller；请求、卡片、模型文本不能提供身份。每个连接/会话只有一个 Host 订阅，转换为标准 ACP 更新及已协商产品更新。

标准 `initialize`、`session/new`、`session/list`、`session/load`、`session/prompt`、`session/cancel` 和 `session/resume` 使用官方 SDK1.4.0。prompt 等待确定终态；`failed` 返回错误，其他结局映射到 ACP stopReason。resource_link 仅转为明确标记的文本引用，不读取 URI、文件或客户端路径。MCP server 注入与多模态输入未提供能力。load 恢复展示历史，resume 调用 Host 的原生续接能力，两者分别失败或成功。两个 resume 入口共用订阅生命周期：停止旧订阅，等待 Host 返回新 Session，再从新水位续订；已有产品 attachment 收到 resync_required 并重新恢复。期间 detach、替换或关闭使未完成的 resume 失去订阅所有权。

产品客户端通过 `clientCapabilities._meta[extension.capability]` 提交 ai-contract 的 `Negotiation`，服务返回所选能力；服务与客户端都强制 selection 是 offer 的子集，未声明的 A2UI 或 false 布尔能力不能重新开启。`_rss-mdm-agent/submit` 返回接纳 receipt；snapshot/list/attach/detach/action/resume 使用同一个产品 schema。未经协商不能调用扩展。A2UI 仅接纳固定版本、catalog 和完整有界恢复消息，失效内容触发重新同步，不作为成功更新发布。

`requestPermission(caller, upstreamRequest, signal)` 仅向该 caller 已附着的会话连接递送标准 ACP 权限问题/选项。它消费组合根持有的实时回调期限，首个有效回答生效，会话取消会终止该 caller/session 的所有待决权限递送，断开仅结束该连接的递送；取消后不恢复回调；它没有权限数据库，也不签发 Rust 执行批准。普通 `Interaction.category=question` 保持独立。

SDK `Stream` 是唯一 transport 接缝；直接提供 `ndJsonStream`。本地/Tauri channel 适配由 ai-client 的 `channelStream` 提供，实际桌面组合根归 #2413。连接关闭只释放订阅和请求，不关闭共享 Host、不取消已接纳命令。HTTP/relay 没有生产实现；独立验收中的 loopback HTTP 仅传送 SDK 测试消息。

运行 `pnpm test:ai-access`、`pnpm check:ai-access-consumer`。版本、边界及消费方法见[开发说明](../../docs/guides/ai-access-development.md)。

`timeoutMs` 限定请求接纳和权限等待预算，不能截断订阅或把 prompt 等待误作终态。标准 prompt 的持久排队期限由独立 `promptTtlMs` 控制，默认 24 小时；超过期限才失效。未知派发以 `reconciliation_required` 错误结束当前 ACP 等待并提示核实，账本继续保留原 attempt，不自动重发。订阅随连接存活，意外结束要求恢复。`await service.close()` 立即停止接入、取消自有订阅/权限递送，并在独立 `shutdownTimeoutMs`（默认5000毫秒）内等待清理，之后拒绝 connect。超时报告 `cleanup_timeout`，返回不代表忽略取消的任务已经终止；组合根负责隔离/终止残留 provider，共享 Host 和已接纳命令仍由它持有。`onDiagnostic(code)` 只输出闭合分类，不包含 caller、模型内容或原始异常；配置 limits 同时约束原始传输 envelope。

客户端 `SessionView.sessionStatus` 独立保留快照的 active/recovery_required/retired 状态，`connection` 只表示传输附着及同步状态。完整 restore 后即使 attached，恢复不可用的会话仍显示 recovery_required；历史恢复事件不能覆盖较新快照中的健康状态。
