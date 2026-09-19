# Codex app-server adapter

`@rss-mdm-agent/ai-adapter-codex` 直接启动固定 `@openai/codex@0.155.0` 的原生 app-server，以 JSON-RPC stdio 消费其 thread/turn/item 协议。它通过 A01 `ProviderInstance` 分离稳定 `ProviderAgentPort`、受控扩展和脱敏诊断端口，不依赖 Git、PR、ACP、UI、SQLite 或执行内核；工作目录由 Host 显式提供。

## 准入与消费

从包主入口导入 `createCodexAdapter`，从 `@rss-mdm-agent/ai-contract/session` 导入 `VerifiedProviderSession`。工厂返回 `{ agent, extensions, diagnostics }`；Host 用 `VerifiedProviderSession.open(instance.agent, configuration, budget)` 准入；失败时检查 `cleanupError`，必要时继续对同一 `instance.agent` 调用 `close`。每个 port 只接受一次 open/restore，关闭后新建实例。

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
| fork | Host 先用工厂持有新的 child instance，再调用 `parentSession.fork(child, throughTurnId, childConfiguration, budget)`；A01 验证同租户/主体/config/account/workspace、新逻辑 session，adapter 验证原生终态 turn |

固定版本实际 fork 返回新的 sessionId 和 threadId，均保存原值；源关系使用 forkedFromId。A01 `ProviderForkPort` 只在 Host 已持有的 child 上执行原生操作，不创建其他 adapter 或反向执行 Host 准入。`VerifiedProviderSession.fork` 复用共同准入与受控 verifier；失败清理 child，`cleanupError` 表示 Host 仍须对同一 `child.agent` 重试 close。创建回执丢失或创建后准入失败返回 unknown，禁止盲目重试创建。Host/A03 持有创建意图、来源与子会话的持久化，成功记录后才对客户端发布；该 adapter 不接管 Host journal。A03/产品装配仍由对应任务交付。

`reconcile` 读取原 thread 的完整分页历史，以 userMessage.clientId 对照原 attempt；查无记录仍为 unknown，绝不自行得出 not_submitted。固定协议明确返回 turn/steer 未提交拒绝时，适配器保留该 incarnation 内的负面证据，submit 返回 not_sent，reconcile 可据此返回 not_submitted；Host 经原 attempt 的验证凭证提交本地失效后继续对话。这不扩展为任意 RPC error 或跨进程负面推断。RPC request ID 不进入持久化身份。原生进程退出、网络错误、取消请求成功均不能伪造模型终态。核实结果经 `VerifiedProviderSession.reconcile` 才能成为 Store 可消费的凭证；adapter 不修改 Host 账本。原生 history 仅供内部 reconcile/fork 核实和 testing；正式端口不返回上游实验 Turn DTO，也不重放已有产品稳定事件。

跨 turn 的 Binding 可清除 run/request；仅当同一 turn 的全部已派发 prompt（含未知 steer）均已核实结束时允许清除。A01、真实 SQLite 恢复测试及 ACP 按 turn 去重取消共用这一规则，不增加另一套恢复状态机。

## 权限与协议边界

启动与恢复都校验配置层、固定版本、真实 cwd、只读且禁止网络的原生 sandbox 以及 required HTTP MCP 的实际连接/目录状态。原生审批策略为 on-request；所有反向审批、动态工具、追问和 elicitation RPC 一律拒绝。内置 shell/exec、文件修改、网络检索、插件、hooks、skills、apps、子代理等入口关闭；不读取个人配置、信任设置或账号目录，原生 stderr 不输出。子进程 PATH 不继承宿主：Unix 固定为 `/usr/bin:/bin`；Windows 使用私有配置目录下的空搜索路径并禁用默认 cwd 查找（Windows 仍需独立平台证据）。同类 Claude adapter 使用相同搜索边界且通过当前 Node 的绝对路径启动。工作目录存在项目配置层会拒绝准入。

唯一业务提案路径是带 incarnation bearer 的 loopback `rss_host.propose`，实际执行权仍由 Host/Rust 持有；MCP 的 approve 配置仅准许调用这个提案接缝，不签发执行批准。Codex 同时暴露三项原生 MCP resource 辅助工具：list 返回空集合，read 始终拒绝，不能访问工作目录或其他资源。重新准入必须重新连接并核验同一封闭目录，不能只依赖 required 标记或缓存。

为显式传入空 environments/runtimeWorkspaceRoots/dynamicTools，握手启用固定版本 experimentalApi；这不暴露通用实验 API。新 thread 的 dynamicTools 为空；只恢复本 adapter 创建的受信 lineage。turn/start 再次显式设置空环境/根目录。恢复/fork 在固定版本会返回本地 cwd 环境，必须精确匹配且无额外 runtime roots。子代理、动态工具、原生终端、结构化追问和多模态均声明 unsupported。

稳定 A01 事件只投影文本、工具提案/结果、错误和明确终态；工具的 returned/rejected/unavailable 保留 Host 原判定。`nativeDiagnostics: true` 仅开启 `instance.diagnostics.observe` 的封闭分类流：text_delta/item_completed/turn_completed/other 与 dropped 计数，不包含原生 method、正文、路径、工具参数或原生标识。诊断缓冲满时丢弃新诊断并累加计数；无人/慢消费不会停止 provider。

## 验证与预算

Host 工具提案通过 A01 `withinBudget` 共享 30 秒 deadline 与实例关闭信号；正常完成、失败、超时或关闭都释放请求的计时器与父 signal 监听，不在长期 bridge 上累积 Node composite signal 依赖。Claude、DeepSeek 工具提案复用同一封装。

```sh
pnpm test:ai-codex
pnpm check:codex-protocol
pnpm check:codex-consumer
pnpm smoke:codex --config-file /absolute/host-owned-config.json
```

测试分为共享 Provider conformance/协议故障 fixture、固定真实进程 + 本地 Responses 流、真实进程权限旁路负测、仓外 tarball 独立消费，以及显式配置端点 smoke。前四项不需要真实模型凭据，不能冒充真实模型或完整产品验收。`tools_disabled` 用例在各平台运行；`host_mediated` 真实行为用例仅在 macOS arm64 运行，其他平台明确验证接纳返回 `unsupported_capability`。POSIX Git shim 负测不作为 Windows 进程证据；Windows 仍运行启动环境断言。

#2405 的完成门是适配器独立验收：隔离目录中的固定 app-server、真实模型多轮与冷恢复、以及可注入的受控工具服务实验，不需要桌面、AI store 或 Rust 执行内核完成。#2413 消费这些组件和控制证据，负责最终桌面组合接线并重跑关键旁路及业务恢复场景；不会反向成为 #2405 的前置。缺少独立模型配置与缺少产品装配是不同的未覆盖项。

smoke JSON 为 `{ "mode": "real_model", "apiUrl": "https://api.openai.com/v1", "apiKey": "HOST_SECRET", "model": "EXACT_RETURNED_MODEL_ID" }`，也支持对应 `RSS_CODEX_SMOKE_MODE/API_URL/API_KEY/MODEL` 环境变量。当前随包 smoke 脚本只为固定 OpenAI HTTPS origin 实现了身份取证；这不是 #2405 对模型供应商或直连方式的要求。其他可信后端可以交付独立等价证据，但只有可配置 URL 或自报模型名不构成身份取证。smoke 专属 relay 用内置信任根验证 TLS、拒绝 redirect，并核对这三轮实际 Responses 回执的 model、completed 状态及不同 response ID，记录脱敏摘要；配置中的精确模型 ID 必须与返回值一致，模型别名也不能静默替换。适配器本身仍支持可信 Host 配置的后端，但其真实模型身份需另外建立可信证明。本地替身必须显式选择 `local_fixture`。运行时生成随机 nonce，只在首轮提供；同进程与 cold resume 的后续 prompt 不带答案，比较完整文本。无配置记录 not_run，不读取个人账号配置。#2405 在可信真实模型证据闭合前保持未完成。

被忽略的 `.local-ci-runs/codex-consumer.json` / `codex-smoke.json` 记录源码 SHA、lock/产物摘要、固定运行时、平台和进程退出事实。源码不干净、行为失败、退出未确认或 real_model 身份验证失败都不能报告 passed；receipt 不包含密钥、端点原文、个人路径或对话原文。真实模型 smoke 由提供明确配置的独立运行补证；C20 产品装配和业务执行不在这些脚本的证明范围。

原生 frame 上限1MiB，单 JSON 文本512KiB，并发 RPC64；稳定事件队列最多1024项/4MiB，诊断队列独立最多1024项/4MiB，未关联通知最多128项/4MiB。每代最多1024条派发记录、累计4MiB命令正文，达到限制后需关闭/恢复新的 generation；单 turn 最多8192个已完成 item。分页历史最多1024 turns/4MiB。稳定事件或可靠性状态溢出停止原生实例并保持未决事实；诊断溢出仅丢弃与计数；Host 持久化后按原 attempt 核实，不静默丢事件。`close` 先停止接纳/工具提案，再终止并等待真实进程退出，失败可用新预算重试；超时不是退出证明。

## 来源

协议生成自固定 Codex 版本并保留 Apache-2.0 LICENSE/NOTICE；本仓适配代码为 MIT。固定源码、生成复现和逐文件改写范围见[来源记录](../../../docs/reference/codex-adapter.md)。
