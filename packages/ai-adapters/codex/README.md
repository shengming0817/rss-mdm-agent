# Codex app-server adapter

`@rss-mdm-agent/ai-adapter-codex` 直接启动manifest 固定的 `@openai/codex` 的原生 app-server，以 JSON-RPC stdio 消费其 thread/turn/item 协议。它通过 A01 `ProviderInstance` 分离稳定 `ProviderAgentPort`、受控扩展和脱敏诊断端口，不依赖 Git、PR、ACP、UI、SQLite 或执行内核；工作目录由 Host 显式提供。

## 准入与消费

从包主入口导入 `createCodexAdapter`，从 `@rss-mdm-agent/ai-contract/session` 导入 `VerifiedProviderSession`。工厂返回 `{ agent, extensions, diagnostics }`；Host 用 `VerifiedProviderSession.open(instance.agent, configuration, budget)` 准入；失败时检查 `cleanupError`，必要时继续对同一 `instance.agent` 调用 `close`。每个 port 只接受一次 open/restore，关闭后新建实例。

`CodexAdapterOptions.resolveConfiguration(identity, budget)` 由可信 Host 返回完全一致的 `CodexConfiguration`、RSS 私有 `nativeDirectory` 与 `authentication`。`api_key` 模式显式提供 API URL/密钥/模型，URL 仅允许 HTTPS 或 loopback HTTP；`existing_config` 模式只指定官方配置目录，以该目录为 CODEX_HOME，登录、凭据读取和刷新由官方 app-server 完成。RSS 不解析外部凭据、不调用账号读取/登录 RPC、不创建个人配置副本。模型省略时使用官方默认值；固定版本 app-server 不支持命名 profile，故不提供 profile 选择。秘密不进入产品 wire。

`nativeDirectory` 是无别名的规范绝对路径、Unix 权限0700，保存 RSS 会话工作目录；自定义 API 模式也用它作为 CODEX_HOME。启动 cwd 使用目录内的空白 runtime-workspace，thread cwd 使用 Host 提供的真实工作目录。已有配置目录及用户配置指定的模型指令由官方工具使用，其原始内容不由 RSS 改写；RSS 的工具权限限制仍显式覆盖并核验。无效配置返回 invalid_input/never，权限或配置层拒绝返回 permission_denied/never；瞬时初始化故障才返回 unavailable，错误不包含配置值。

恢复或 fork 时 resolver 另收到 `history: Binding`。Host 必须先查可信持久化绑定，核验原 session/thread 属于这一 namespace/config/workspace 的 adapter 历史，再返回相等的 `ownedHistory: { nativeSessionId, nativeThreadId }`。直接回显用户输入或原始 history 参数不构成所有权证明。禁止导入任意 Codex 历史、路径或手工编辑的 rollout。

`tools_disabled` 不创建工具桥。`host_mediated` 需要 A01 `ToolEndpoint` 和可信 `ToolVerifier`；当前只在 macOS arm64 的固定版本提供受控准入，其他平台返回 unsupported。Verifier 仍需对当前 incarnation 签发证明，不能用本包版本声明代替 Host 准入。真实平台扩展由单独证据支持。

## 恢复

原生身份与产品命令分开保存，Host 先持久化派发意图。fork 只从已核验的终态历史创建新上下文，失败不盲目重试创建。具体映射见 [src](src/)。

`reconcile` 读取原 thread 的完整分页历史，以 userMessage.clientId 对照原 attempt；查无记录仍为 unknown，绝不自行得出 not_submitted。固定协议明确返回 turn/steer 未提交拒绝时，适配器保留该 incarnation 内的负面证据，dispatch 返回 not_sent，reconcile 可据此返回 not_submitted；Host 经原 attempt 的验证凭证提交本地失效后继续对话。这不扩展为任意 RPC error 或跨进程负面推断。RPC request ID 不进入持久化身份。原生进程退出、网络错误、取消请求成功均不能伪造模型终态。核实结果经 `VerifiedProviderSession.reconcile` 才能成为 Store 可消费的凭证；adapter 不修改 Host 账本。原生 history 仅供内部 reconcile/fork 核实和 testing；正式端口不返回上游实验 Turn DTO，也不重放已有产品稳定事件。

跨 turn 的 Binding 可清除 run/request；仅当同一 turn 的普通 prompt 已有终态、所有 steer 均已确认或核实未提交时允许清除。A01、真实 SQLite 恢复测试及 ACP 按 turn 去重取消共用这一规则，不增加另一套恢复状态机。

## 权限与协议边界

启动与恢复都校验配置层、固定版本、真实 cwd、只读且禁止网络的原生 sandbox 以及 required HTTP MCP 的实际连接/目录状态。原生审批策略为 on-request；所有反向审批、动态工具、追问和 elicitation RPC 一律拒绝。内置 shell/exec、文件修改、网络检索、插件、hooks、skills、apps、子代理等入口关闭；官方已有用户配置可以复用，但继承的 MCP 入口由官方 config/read 枚举并关闭，hooks/skills/plugins 和原生执行能力显式禁用，原生 stderr 不输出。子进程 PATH 不继承宿主：Unix 固定为 `/usr/bin:/bin`；Windows 使用私有配置目录下的空搜索路径并禁用默认 cwd 查找（Windows 仍需独立平台证据）。同类 Claude adapter 使用相同搜索边界且通过当前 Node 的绝对路径启动。工作目录存在项目配置层会拒绝准入。

唯一业务提案路径是带 incarnation bearer 的 loopback `rss_host.propose`，实际执行权仍由 Host/Rust 持有；MCP 的 approve 配置仅准许调用这个提案接缝，不签发执行批准。Codex 同时暴露三项原生 MCP resource 辅助工具：list 返回空集合，read 始终拒绝，不能访问工作目录或其他资源。重新准入必须重新连接并核验同一封闭目录，不能只依赖 required 标记或缓存。

为显式传入空 environments/runtimeWorkspaceRoots/dynamicTools，握手启用固定版本 experimentalApi；这不暴露通用实验 API。新 thread 的 dynamicTools 为空；只恢复本 adapter 创建的受信 lineage。turn/start 再次显式设置空环境/根目录。恢复/fork 在固定版本会返回本地 cwd 环境，必须精确匹配且无额外 runtime roots。子代理、动态工具、原生终端、结构化追问和多模态均声明 unsupported。

稳定 A01 事件只投影文本、工具提案/结果、错误和明确终态；工具的 returned/rejected/unavailable 保留 Host 原判定。`nativeDiagnostics: true` 仅开启 `instance.diagnostics.observe` 的封闭分类流：text_delta/item_completed/turn_completed/other 与 dropped 计数，不包含原生 method、正文、路径、工具参数或原生标识。诊断缓冲满时丢弃新诊断并累加计数；无人/慢消费不会停止 provider。

## 验证与预算

Host 工具提案通过 A01 `withinBudget` 共享 30 秒 deadline 与实例关闭信号；正常完成、失败、超时或关闭都释放请求的计时器与父 signal 监听，不在长期 bridge 上累积 Node composite signal 依赖。Claude、DeepSeek 工具提案复用同一封装。

```sh
pnpm test:ai-codex
pnpm check:codex-protocol
pnpm smoke:codex --config-file /absolute/host-owned-config.json
```

本地协议与真实原生进程测试不代表外部模型或完整产品验收。

smoke JSON 为 `{ "mode": "real_model", "apiUrl": "https://api.openai.com/v1", "apiKey": "HOST_SECRET", "model": "EXACT_RETURNED_MODEL_ID" }`，也支持对应 `RSS_CODEX_SMOKE_MODE/API_URL/API_KEY/MODEL` 环境变量。当前随包 smoke 脚本只为固定 OpenAI HTTPS origin 实现了身份取证；这不是 #2405 对模型供应商或直连方式的要求。其他可信后端可以交付独立等价证据，但只有可配置 URL 或自报模型名不构成身份取证。smoke 专属 relay 用内置信任根验证 TLS、拒绝 redirect，并核对这三轮实际 Responses 回执的 model、completed 状态及不同 response ID，记录脱敏摘要；配置中的精确模型 ID 必须与返回值一致，模型别名也不能静默替换。适配器本身仍支持可信 Host 配置的后端，但其真实模型身份需另外建立可信证明。本地替身必须显式选择 `local_fixture`。运行时生成随机 nonce，只在首轮提供；同进程与 cold resume 的后续 prompt 不带答案，比较完整文本。无配置记录 not_run，不读取个人账号配置。

smoke 记录运行结果、进程退出与未覆盖项，不包含秘密或对话原文。行为失败、退出未确认或真实模型身份验证失败均不能通过。关闭超时保留待清理状态，使用新预算重试。

## 来源

协议生成自固定 Codex 版本并保留 Apache-2.0 LICENSE/NOTICE；本仓适配代码为 MIT。固定源码、生成复现和逐文件改写范围见[来源记录](../../../docs/reference/codex-adapter.md)。

公共 `ProviderConfiguration` 仅含可序列化身份与配置；受控工具通过 adapter options 的 `tools` 注入，并将同一个 endpoint 与可信 verifier 作为 `ProviderAdmission` 显式传入 `VerifiedProviderSession.open/restore/fork`。所有命令共用带 attempt 的 `dispatch`；cancel/respond（以及支持的 steer）成功返回 `acknowledged`，不生成模型 outcome。原生核实证据经 `SessionCommit.providerFacts` 原子提交。
