# Claude Agent SDK adapter

`@rss-mdm-agent/ai-adapter-claude` 消费 A01 `ProviderAgentPort`，使用完整官方
`@anthropic-ai/claude-agent-sdk@0.3.277`（随包 CLI `2.1.277`）。Node.js ≥24。
不启动系统 `claude -p`，不依赖 Tauri、SQLite、执行内核或相邻仓库。

```ts
import { createClaudeAdapter } from '@rss-mdm-agent/ai-adapter-claude';
import { VerifiedProviderSession } from '@rss-mdm-agent/ai-contract/session';

const configuration = {
  namespace: { tenantId:'tenant', principalId:'principal', authorityId:'authority', sessionId:'session' },
  provider: 'claude', config: { id: 'tenant-config', revision: '1' },
  accountRef: 'account-ref', workingDirectory: '/absolute/workspace',
  permissions: 'tools_disabled',
} as const;
const adapter = createClaudeAdapter({
  resolveConfiguration: async (identity, budget) => {
    // Composition resolves the exact immutable config/account refs from its secret store.
    const secret = await resolveTrustedSecret(identity, budget);
    return { configuration, configurationDirectory: secret.nativeContextDirectory,
      apiUrl: secret.apiUrl, credential: { type: 'api_key', value: secret.apiKey } };
  },
});
const budget = { timeoutMs: 30000, signal: new AbortController().signal };
const session = await VerifiedProviderSession.open(adapter, configuration, budget);
```

`resolveTrustedSecret` 是组合根自己的函数。resolver 不得从模型内容、普通 Command 或未经认证的
JSON 取得凭据。`ClaudeConfiguration` 将 provider 固定为 claude，resolver 不能返回其他引擎。
支持 `api_key` 或 `auth_token` 二选一，分别进入子进程
`ANTHROPIC_API_KEY` / `ANTHROPIC_AUTH_TOKEN`；URL 仅允许 HTTPS 或本机 HTTP 测试端点。
凭据不进入 Binding、observation、诊断、版本引用。原生配置目录由组合根按账号/配置隔离，
须持久保留用于恢复、限制本机访问；本包不读取用户全局 Claude 配置。

## 生命周期与数据

- `createSession` 为 `query` 显式指定 UUID；`resume` 只使用指定的 `nativeSessionId`，
  校验固定 provider/adapter 版本并生成新 generation，不使用目录的“最近会话”。
  Host 调用 `VerifiedProviderSession.restore(adapter, previousSession, configuration, budget)`，
  显式消费当前配置并对新 generation 重新验证；返回新的名义准入，旧证据不能复用。
  adapter 的原始 resume 只返回待验证的 binding+capabilities，不能代替这个入口。
- 一次只派发一个 turn。Host 持有持久 command ledger 与 `queue_next` 队列；忙时返回
  `not_sent/same_command`，provider 的 queue 能力明确为 unsupported。Host 可另行持有队列。本包保留有界进程内 command 身份，不声明持久接纳。
- 本地 input 入队不是提交确认。原生回显/带匹配 prompt UUID 的回复才确认 submitted。
  `nativeRequestId` 是送入 SDK 并由其回显的 user-message UUID；SDK 未提供独立 run ID，
  因而不构造 `nativeRunId`。已交给 SDK 但未确认的请求返回 unknown，先 reconcile。
- SDK 原生 transcript 持有模型上下文；Host 文本事件只用于展示。冷恢复不导入展示记录，
  不复活旧 generation 的 callback，不凭没有观测到结果推断“未执行”。
- delta 使用原生 message ID；稳定文本和匹配 UUID 的原生 result 分别映射展示与模型终态。
  原生 aborted result 映射 cancelled，max_tokens 映射同名终态，error_max_turns 映射 max_turn_requests；美元预算/结构化输出重试耗尽映射 failed，不伪装 token 或 turn 上限。
  不转发 thinking、原始 SDK 异常、原始 stderr。断流/超时无终态；取消请求本身仍仅 request_only。
- `close` 结束输入与 callback、调用 SDK close；只有真实 child exit 或确证未 spawn 才确认
  processStopped。超时可带新预算重试。结束模型 turn 不等于进程退出或业务执行成功。

## 工具与问题

原生执行工具关闭。允许的原生工具仅 `AskUserQuestion`；其回答只恢复问题回调。
受控模式另注册 SDK 进程内 `rss_host.propose` 工具，唯一接缝是 A01 `ToolEndpoint.propose`。
它只转交不可信提案与结果，不能签发批准/执行许可。

`host_mediated` 必须由组合根提供 ToolEndpoint 与可信 verifier，并通过
`VerifiedProviderSession.open` / `VerifiedProviderSession.restore` 准入。verifier 的证据必须适用于当前版本、平台、配置和
incarnation；测试 verifier 不能用于生产。SDK 策略限制不是 OS 沙箱证明。

设置来源、skills、plugins、MCP 配置和 child env 显式收窄；没有任意 SDK options 透传。
PreToolUse 对未知工具拒绝，对受控问题/桥强制 ask；canUseTool 再检查精确工具和 SDK MCP
provenance。没有 permission bypass、自动持久 permission rule 或“普通回答就是批准”。
原生 slash prompt 与 expansion 禁用，防止会话/权限模式被命令改变。

Interaction 固定 `category: "question"`，只有问题可进入普通 respond。`nativeCallbackId` 来自 SDK `requestId`，与父 prompt UUID 独立；callback
只驻留内存，最多 32 个/turn，默认 120 秒（可设 1–600000ms）。回应严格为
`{ answers: { '<question>': '<answer>' } }`，必须覆盖全部原问题；额外权限字段拒绝。
重复相同响应 command 幂等，不同响应返回 already_answered；过期、signal abort、关闭和
冷恢复后旧 callback 不可回答。未知 MCP elicitation 与 user dialog 返回取消。

## 能力与验证

声明 continuation=across_processes、cancellation=request_only、structuredQuestion=supported；
steer、fork、subagent、terminal facility、multimodal 均 unsupported。
受控工具能力仍受可信平台 verifier 门控，不以本机测试替代其他平台证明。

```sh
pnpm test:ai-claude          # SDK fixtures + 真实 SDK / 固定 HTTP 模型传输
pnpm check:claude-consumer  # 隔离目录 pack/install/types/公共 API 和真实 SDK 接缝
pnpm smoke:claude           # 配置端点 smoke；读取环境 URL + 单一凭据
```

配置端点 smoke 也可显式 `--credential-file PATH` 读取该文件的 env 中 URL/凭据/model；
不加载该文件的 hooks、权限或信任设置。结果写入被忽略的 `.local-ci-runs/claude-model.json`，
记录源码 SHA、lock hash、SDK/CLI/Node/平台、凭据模式、端点 origin 哈希、请求模型
（未配置时为 provider_default）、结果与未覆盖项，不记录密钥、私有 URL 或对话原文。
证据类型为 real-sdk-configured-endpoint-smoke；兼容端点或代理的响应本身无法证明
上游实际模型身份，backendIdentityVerified 始终为 false，不据此宣称真实 Claude 后端验证。
失败证据保留闭合的阶段和 Result/Submission/Outcome 分类。关闭未确认时最多用新预算
重试一次；仍未停止则保留运行目录，并在 receipt 标记清理状态，避免删除仍在使用的目录。
交付证据要求已提交且干净的源码。完整本地 CI 不需要模型凭据，只运行确定性模型传输。

固定传输测试证明真实 SDK streaming、同进程/跨进程上下文、取消/HTTP 错误、AskUserQuestion 往返和
恶意 settings/skills/MCP 旁路拒绝；它不证明真实模型行为。配置端点 smoke 另验新建、
续聊和冷恢复；业务执行与完整产品装配属于其他 owner。

会话最多保留 256 条紧凑派发记录，达到上限后需 close/resume 开启新 generation；
已完成的 prompt/answer 原文不由幂等记录保留。待消费展示队列单 turn 上限 2 MiB、
全 session 共用 4 MiB，消费后释放预算；溢出进入 unknown 并停止原生进程，不能伪造终态。
模型 turn 数上限 32。金额预算与账单政策由消费产品持有，本包不声明费用上限；
错误只投影 value-free Result/Outcome，不透传 SDK stderr 或底层错误正文。

## 来源与许可

实现为本仓 TypeScript 重写，未复制 prmonitor 代码。对标
`prmonitor@4dcc87264ad740da6559824e0a8b04a1c2914d4b` 的 Claude CLI 生命周期/来源边界；
其 Rust CLI 调用、PR 业务、Tauri/SQLite、旧 bypass/自动批准方式均未迁入。

| 固定参考文件 | 本包落点与改写边界 |
|---|---|
| `src-tauri/src/review/engines/claude/engine.rs`、`manager.rs` | `src/adapter.ts`：仅对标生命周期/续接差异；改为 A01 port 和原生 SDK，无 review 任务语义 |
| `src-tauri/src/review/engines/claude/process.rs` | `src/runtime.ts`：进程结束证据概念；不复制 CLI 参数/stdio 解析或旧权限设置 |
| `src-tauri/src/review/session.rs` | Host 展示与原生 context 分离的参考；不迁移旧 session store，SDK 自有 transcript |

SDK `createSdkMcpServer` 在固定版本对 Zod record 的 bundled schema 转换会失败；桥参数
使用等价的开放 object schema，真实 SDK 首轮工具清单与调用测试覆盖该接缝。`alwaysLoad`
只保证唯一桥及时进入模型工具集，不增加自动许可。

一手固定参考：[@anthropic-ai/claude-agent-sdk@0.3.277 sdk.d.ts](https://unpkg.com/@anthropic-ai/claude-agent-sdk@0.3.277/sdk.d.ts)
的 Query、Options、SDKMessage、CanUseTool、PreToolUse、SpawnOptions；
[sdk.mjs](https://unpkg.com/@anthropic-ai/claude-agent-sdk@0.3.277/sdk.mjs) 的进程、MCP 与回调实现。
本包新代码 MIT；Anthropic SDK/CLI 保留其自身商业许可及使用条款，未重标为 MIT。
lock 固定 SDK 及平台包版本；pnpm release-age 例外仅列这些精确版本，未关闭全局供应链检查。


A01 恢复契约升级后，submit 显式接收 Host 已持久化的 DispatchAttempt；重复调用必须保持原 attempt。所有观察含 attemptId，reconcile 接收完整 CommandRecord 并返回同一命令/attempt 的闭合结果；Host 通过 VerifiedProviderSession.reconcile 获取 store 可消费的 nominal 证据。原生 echo 使用 submitted 观察，callback 丢失使用 interaction_unavailable 观察；adapter 不直接发布 Host 的 status/interaction 生命周期事件。Binding 持久化 workspaceId，resolver 必须与准入 namespace 和工作目录一致；每个 adapter 实例在 close 后永久停止准入，冷恢复使用新实例。


A03 统一端口：prompt、cancel 与 respond 均调用 `dispatch(binding, command, attempt, budget)`。cancel / respond 返回 acknowledged，不形成模型 Outcome。原生 acceptance 只证明 submitted；assistant / stream_event 的运行观察才证明 running。`ProviderConfiguration` 只含数据，ToolEndpoint 由 `ClaudeAdapterOptions.tools` 注入；verifier 由 parent 的 `VerifiedProviderSession` 准入参数持有，adapter / SDK worker 不接收 verifier。

原生 transcript/恢复状态目录在 SDK 启动前校验为当前 UID 拥有的真实私有目录，拒绝 symlink、普通文件及任何 group/other 权限。缺失目录按 `0700` 创建，`CLAUDE_CONFIG_DIR` 只接收核对过的 canonical realpath；错误不包含路径或凭据。
