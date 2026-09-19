# C12 Codex 来源与改写

本包对应 #2405，直接对接 OpenAI 官方 app-server，不迁移 prmonitor 的 PR runner、权限默认值、数据库或任务提示。适配器为本仓 TypeScript 实现，使用 A01 的既有准入/派发/恢复契约；不使用通用 Codex SDK 或 ACP facade 代替原生协议。

固定上游为 `openai/codex` revision `f0a1b8f0849d90960bc406b848f32e5a129b0457`（npm `@openai/codex@0.155.0`）。本次直接核对固定源码：

| ref: OpenAI Codex file | 采用机制与本包落点 |
| --- | --- |
| [app-server-protocol/src/protocol/v2/mod.rs](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/app-server-protocol/src/protocol/v2/mod.rs) | thread/turn/item、MCP 状态、初始化参数；`src/protocol` 保存生成类型，`src/protocol.ts` 封闭实际调用方法 |
| [app-server/src/request_processors/thread_processor.rs](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/app-server/src/request_processors/thread_processor.rs) | start/resume/fork 的配置和响应；`src/adapter.ts` 验证原生身份与 lineage，不推导 fork 的 sessionId |
| [core/src/session/session.rs](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/core/src/session/session.rs) | clientUserMessageId、活动 turn、动态工具与恢复语义；改写为 Host 已持久化 attempt 的映射，不声称服务端幂等 |
| [core/src/config/mod.rs](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/core/src/config/mod.rs) | 配置层与严格配置；`src/configuration.ts` 生成独占 CODEX_HOME，拒绝外部配置层、个人信任及继承环境 |
| [tools/src/tool_spec.rs](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/tools/src/tool_spec.rs) | 原生工具开关与 MCP resource 辅助项；封闭 built-ins，仅允许宿主提案与空资源目录 |

明确 steer 拒绝的负面证据还核对了 [app-server 的 turn_processor.rs](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/app-server/src/request_processors/turn_processor.rs) 与 [core/session/turn_input.rs](https://github.com/openai/codex/blob/f0a1b8f0849d90960bc406b848f32e5a129b0457/codex-rs/core/src/session/turn_input.rs)：所采用的 NotSubmitted 拒绝发生在输入入队之前。适配器仅保留固定方法/版本/错误形状的封闭证据，不把内部错误、断线或超时当成未提交。

`scripts/generate-codex-protocol.mjs` 调用安装的固定原生程序 `app-server generate-ts --experimental`，只保存所用请求/响应的传递依赖闭包。转换限于 NodeNext import 后缀和统一格式，不手写上游字段或改协议语义。`protocol-manifest.json` 保存 revision、实验字段用途和逐文件 SHA-256；`pnpm check:codex-protocol` 重新生成并逐字节核验，额外文件也拒绝。write 模式先暂存完整闭包与 manifest，再发布整棵树、删除旧闭包；发布失败恢复旧树。check 只读，检查文件集合和逐字节摘要。运行时独立验证包/平台包版本，不通过 PATH 选择另一个 CLI。

上游生成文件遵守 Apache-2.0；随包包含原始 `protocol-LICENSE` 和 `protocol-NOTICE`。包根 `LICENSE` 仅适用于本仓新增 MIT 实现；没有把上游二进制或生成协议重新标为 MIT。MCP 使用固定 `@modelcontextprotocol/sdk@1.30.0` 的官方 Streamable HTTP server API。

固定真实进程测试提供原生流、恢复、fork、steer、取消、HTTP 错误、MCP 接入与旁路拒绝的证据；Responses 本地测试服务不是真实模型。独立消费/模型 smoke 的源码与产物摘要保存在可再生本地 receipt，最终结果随 PR 留痕。macOS arm64 之外的受控模式、真实模型和产品装配必须分别补证，不能从生成类型或一个平台外推。

PR #1058 本轮修复新增核对：Rust std `process::Command` 的 PATH 查找与 `env_clear`；固定 Codex `core/src/git_info` 的原生命令查找；[tracing-appender non_blocking.rs](https://github.com/tokio-rs/tracing/blob/tracing-appender-0.2.3/tracing-appender/src/non_blocking.rs) 的 lossy/drop counter。采用封闭进程搜索路径、诊断丢弃计数与 A01 三端口，未引入对应 Rust 依赖。恶意 git shim 在真实 Git 工作目录/固定 app-server 上先复现、后拒绝执行。

真实模型 smoke 参考 [OpenAI Responses 回执](https://developers.openai.com/api/reference/typescript/resources/beta/subresources/responses/methods/create) 的 response.completed/model/id/status，仅将固定 TLS peer 实际返回的模型身份作为可信后端声明；不以请求模型名或模型回答自报身份。非官方转发网关不在该验收入口的信任范围。未提供明确凭据时 real_model 为 not_run，#2405 不自动关闭；本地替身的 nonce 连续性正反例只证明协议与脚本判定。
