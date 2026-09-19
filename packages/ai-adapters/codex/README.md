# Codex app-server adapter

`@rss-mdm-agent/ai-adapter-codex` 直接启动固定 `@openai/codex@0.155.0` 的原生 app-server，以 JSON-RPC stdio 消费其 thread/turn/item 协议。它实现 A01 `ProviderAgentPort`，不依赖 Git、PR、ACP、UI、SQLite 或执行内核；工作目录由 Host 显式提供。

## 准入与消费

从包主入口导入 `createCodexAdapter`，从 `@rss-mdm-agent/ai-contract/session` 导入 `VerifiedProviderSession`。Host 用 `VerifiedProviderSession.open(port, configuration, budget)` 准入；失败时检查 `cleanupError`，必要时继续对同一 port 调用 `close`。每个 port 只接受一次 open/restore，关闭后新建实例。

`CodexAdapterOptions.resolveConfiguration(identity, budget)` 必须由可信 Host 实现，返回与请求完全一致的 `CodexConfiguration`、私有 `nativeDirectory`、Responses API 的 `apiUrl`、`apiKey` 和 `model`。URL 只允许 HTTPS 或本地 loopback HTTP；凭据不进入产品 wire。无效配置返回 invalid_input/never，权限或配置层拒绝返回 permission_denied/never；瞬时初始化故障才返回 unavailable，错误不包含配置值。`nativeDirectory` 必须是无别名的规范绝对路径，Unix 权限0700，只归本 adapter/config/account/workspace 的受信 lineage 使用，不能与个人 Codex CLI、其他应用或不同配置共用。适配器以该目录作为 CODEX_HOME，启动 cwd 使用目录内的空白 runtime-workspace，thread cwd 使用 Host 提供的真实工作目录。

恢复或 fork 时 resolver 另收到 `history: Binding`。Host 必须先查可信持久化绑定，核验原 session/thread 属于这一 namespace/config/account/workspace 的 adapter 历史，再返回相等的 `ownedHistory: { nativeSessionId, nativeThreadId }`。直接回显用户输入或原始 history 参数不构成所有权证明。禁止导入任意 Codex 历史、路径或手工编辑的 rollout。

`tools_disabled` 不创建工具桥。`host_mediated` 需要 A01 `ToolEndpoint` 和可信 `ToolVerifier`；当前只在 macOS arm64 的固定版本提供受控准入，其他平台返回 unsupported。Verifier 仍需对当前 incarnation 签发证明，不能用本包版本声明代替 Host 准入。真实平台扩展由单独证据支持。

## 命令与原生身份

| 产品坐标/操作 | 原生对应与边界 |
| --- | --- |
| Binding.nativeSessionId / nativeThreadId | 分别保存原生 sessionId / thread.id，禁止互相推导；恢复必须同时匹配 |
| DispatchAttempt.attemptId | Host 先持久化 intent，随后作为 clientUserMessageId 发给 Codex；它只是关联标识，不提供服务端幂等保证 |
| nativeRunId / nativeRequestId | turn.id / 已确认接纳的 clientUserMessageId；每个 start/steer 保留自己的 attempt/request |
| queue_next prompt | 一个未决普通 prompt；unknown 也阻止下一次普通提交，本包不持有产品队列 |
| steer prompt | 仅当前已确认的活动 turn；同 turn 的输出属于普通 prompt，明确终态结束所有已确认 start/steer |
| cancel | turn/interrupt 只返回 request_only；必须观察 interrupted 或核实原生历史才能确认取消 |
| resume | 新 port + `VerifiedProviderSession.restore`；Host 消费恢复证据后 rebind，再逐条核实原账本 |
| fork | `port.fork(binding, throughTurnId, childConfiguration, budget)`；必须明确已结束的来源 turn，同租户/主体/config/account/workspace、新逻辑 session；返回新 port/session 和来源坐标 |

固定版本实际 fork 返回新的 sessionId 和 threadId，均保存原值；源关系使用 forkedFromId。fork 创建回执丢失返回 unknown，不重试创建；调用方保留不确定状态并核实，不能把本地 correlationId 当原生查重键。`cleanupPort`/`cleanupError` 表示尚未确认停止的子实例。

`reconcile` 读取原 thread 的完整分页历史，以 userMessage.clientId 对照原 attempt；查无记录仍为 unknown，绝不自行得出 not_submitted。固定协议明确返回 turn/steer 未提交拒绝时，适配器保留该 incarnation 内的负面证据，submit 返回 not_sent，reconcile 可据此返回 not_submitted；Host 经原 attempt 的验证凭证提交本地失效后继续对话。这不扩展为任意 RPC error 或跨进程负面推断。RPC request ID 不进入持久化身份。原生进程退出、网络错误、取消请求成功均不能伪造模型终态。核实结果经 `VerifiedProviderSession.reconcile` 才能成为 Store 可消费的凭证；adapter 不修改 Host 账本。`readHistory` 返回原生上下文视图，不重放已有产品稳定事件。

跨 turn 的 Binding 可清除 run/request；仅当同一 turn 的全部已派发 prompt（含未知 steer）均已核实结束时允许清除。A01、真实 SQLite 恢复测试及 ACP 按 turn 去重取消共用这一规则，不增加另一套恢复状态机。

## 权限与协议边界

启动与恢复都校验配置层、固定版本、真实 cwd、只读且禁止网络的原生 sandbox 以及 required HTTP MCP 的实际连接/目录状态。原生审批策略为 on-request；所有反向审批、动态工具、追问和 elicitation RPC 一律拒绝。内置 shell/exec、文件修改、网络检索、插件、hooks、skills、apps、子代理等入口关闭；不读取个人配置、信任设置或账号目录，原生 stderr 不输出。工作目录存在项目配置层会拒绝准入。

唯一业务提案路径是带 incarnation bearer 的 loopback `rss_host.propose`，实际执行权仍由 Host/Rust 持有；MCP 的 approve 配置仅准许调用这个提案接缝，不签发执行批准。Codex 同时暴露三项原生 MCP resource 辅助工具：list 返回空集合，read 始终拒绝，不能访问工作目录或其他资源。重新准入必须重新连接并核验同一封闭目录，不能只依赖 required 标记或缓存。

为显式传入空 environments/runtimeWorkspaceRoots/dynamicTools，握手启用固定版本 experimentalApi；这不暴露通用实验 API。新 thread 的 dynamicTools 为空；只恢复本 adapter 创建的受信 lineage。turn/start 再次显式设置空环境/根目录。恢复/fork 在固定版本会返回本地 cwd 环境，必须精确匹配且无额外 runtime roots。子代理、动态工具、原生终端、结构化追问和多模态均声明 unsupported。

稳定 A01 事件只投影文本、工具提案/结果、错误和明确终态；工具的 returned/rejected/unavailable 保留 Host 原判定。高级原生通知仅在 `nativeDiagnostics: true` 时通过独立 `codex.native` 流提供，可能含敏感内容，不应默认记录。

## 验证与预算

```sh
pnpm test:ai-codex
pnpm check:codex-protocol
pnpm check:codex-consumer
pnpm smoke:codex --config-file /absolute/host-owned-config.json
```

测试分为共享 Provider conformance/协议故障 fixture、固定真实进程 + 本地 Responses 流、真实进程权限旁路负测、仓外 tarball 独立消费，以及显式配置端点 smoke。前四项不需要真实模型凭据，不能冒充真实模型或完整产品验收。

smoke JSON 为 `{ "mode": "real_model", "apiUrl": "https://model.example/v1", "apiKey": "HOST_SECRET", "model": "HOST_MODEL" }`，也支持对应 `RSS_CODEX_SMOKE_MODE/API_URL/API_KEY/MODEL` 环境变量。真实端点要求非 loopback HTTPS；本地替身必须显式选择 `local_fixture`。脚本验证新会话、连续 turn 和 cold resume，只从该配置读取模型接入信息。无配置记录 not_run，不读取个人账号配置；后端代理的实际模型身份仍未被证明。

被忽略的 `.local-ci-runs/codex-consumer.json` / `codex-smoke.json` 记录源码 SHA、lock/产物摘要、固定运行时、平台和进程退出事实。源码不干净、行为失败或退出未确认都不能报告 passed；receipt 不包含密钥、端点原文、个人路径或对话原文。真实模型 smoke 由提供明确配置的独立运行补证；C20 产品装配和业务执行不在这些脚本的证明范围。

原生 frame 上限1MiB，单 JSON 文本512KiB，并发 RPC64；事件队列各自最多1024项/4MiB，未关联通知最多128项/4MiB。每代最多1024条派发记录、累计4MiB命令正文，达到限制后需关闭/恢复新的 generation；单 turn 最多8192个已完成 item。分页历史最多1024 turns/4MiB。溢出停止原生实例并保持未决事实；Host 持久化后按原 attempt 核实，不静默丢事件。`close` 先停止接纳/工具提案，再终止并等待真实进程退出，失败可用新预算重试；超时不是退出证明。

## 来源

协议生成自固定 Codex 版本并保留 Apache-2.0 LICENSE/NOTICE；本仓适配代码为 MIT。固定源码、生成复现和逐文件改写范围见[来源记录](../../../docs/reference/codex-adapter.md)。
