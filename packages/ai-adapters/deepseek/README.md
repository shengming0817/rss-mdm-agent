# DeepSeek Harness adapter

单包 Node.js / TypeScript 适配器，公开 `createDeepSeekAdapter(options): ProviderAgentPort`。消费 A01 的 `DispatchAttempt` 和 `VerifiedProviderSession`；每个 port 只允许一次 open/restore，独占一个 Node 子进程与原生 Context。公共配置、namespace、账号引用和存储目录由可信组合根提供。协议为 chat-completions；可信 resolver 必须显式提供 HTTPS API endpoint（loopback 测试可用 HTTP），smoke 默认使用官方 `https://api.deepseek.com`。密钥由 resolver 返回，仅通过私有 IPC 进入进程内存。

```ts
import { createDeepSeekAdapter } from '@rss-mdm-agent/ai-adapter-deepseek';
import { VerifiedProviderSession } from '@rss-mdm-agent/ai-contract/session';
const port = createDeepSeekAdapter({
  resolveConfiguration: async (identity, budget) => ({
    configuration, // DeepSeekConfiguration，必须含可信 namespace
    persistenceDirectory, // 私有耐久目录，不能由模型或终端用户任意指定
    model: 'deepseek-chat',
    apiUrl: "https://custom.example.test/v1",
    apiKey: await credentials.resolve(identity.config, budget),
  }),
});
const admitted = await VerifiedProviderSession.open(port, configuration, budget);
// 失败保留 admitted.cleanupError；结束时以新预算调用 port.close。
```

`workspaceIdentity` 只表示公共契约中的逻辑工作目录身份。它不证明操作系统隔离。受控模式仍要求消费方提供 `ControlledToolVerifier`，绑定该 incarnation、平台和 `ToolEndpoint`；本包不提供通用批准器或生产平台认证。配置引用、模型、存储根或静态装配改变后不得恢复旧原生 session。

| 能力 | 映射与边界 |
| --- | --- |
| 提交 | Host 先持久化 intent，再调用 `dispatch(binding, command, attempt, budget)`。原生 requestId 是 namespace/session/command/attemptId 的 SHA-256，accepted 映射 `submitted`，不代表耐久完成 |
| 输出 | Gateway follow 的原生 assistant-stream 提供真实 delta；`assistant/message` 提供稳定文本；原生 `turn/end` flush 后提供终态。断流/进程退出不合成终态 |
| 取消 | `request_only`；只有原生结束事实才能返回 completed/cancelled 等 outcome |
| 问题 | 唯一基础工具 `ask_user_question`；回答格式 `{answers:[{id,selected:[label],custom?}]}`。回调为 `generation_bound`；过期/取消报告 `interaction_unavailable` |
| 受控提案 | 仅增加 `host_propose({name,arguments})` → `ToolEndpoint.propose`，结果只是模型可见文本；无执行许可或 Rust Evidence 写入 |
| 禁止能力 | shell、terminal、MCP、PTC、委派、动态插件、settings/热更新、workspace 指令执行均不装配；原生权限审批无 provider，默认拒绝 |
| 静态约束 | 固定工具定义与来源，monotonic guard 拒绝嵌套/其它 Agent 调用；工具集合变化使 incarnation 失效 |
| 耐久性 | 新会话准入前先 flush 原生身份，支持零消息时关闭并冷恢复；发布包 checkpoint policy 在模型调用和工具体前等待 flush；适配器另在发送终态前等待 flush |

恢复必须遵循 A01 流程：

1. 新建 port，`VerifiedProviderSession.restore(port, previousSession, configuration, budget)` 重新准入。
2. 恢复仅装配服务并 `inspect` 原生历史，不调用 follow/resume、不创建 Agent、不请求模型、不修复原生日志。
3. Host 使用恢复凭证 `SessionStore.rebind`，重新读取 Session 与 CommandRecord。
4. `admitted.reconcile(currentSession, record, budget)` 返回私有 `VerifiedProviderFact`，Host 通过 `SessionCommit.providerFacts` 原子提交。
5. 已结算的耐久终态按 exact previous request 只读核实后可继续新命令；缺失历史、inbox 未完成、冷读补出的 `interrupted` 都保持 unknown，持续阻断；本适配器不从日志缺失推导 `not_submitted`。后续显式 dispatch 才在同一已准入 Context 激活原生 Agent。

原 attemptId、originGeneration、correlationId 和已持久化原生坐标保留，observerGeneration 随重绑变化。适配器只发原生事实；交互/surface 的原子失效、合法重试和命令期限由 Host/Store 持有。该 API 不能感知 Host 是否已提交凭证，调用顺序由 Host 执行。

静态服务声明在 `src/assembly.ts`，同时驱动启动和组合摘要；不加载通用 RPC 或 Loader。私有 IPC 固定为 initialize/prompt/inspect/cancel/answer/tool_result/close。原生 Connection 仅为服务注册依赖，不启动网络 listener。临时 Harness home 含其自身 browser-session secret，只有子进程退出且目录清理成功才报告停止。API key 不写入该目录。close 强制终止本 incarnation，未 flush 的原生事实可能丢失，因此未完成命令必须核实。

验证入口：`pnpm test:ai-deepseek`（A01 fixture + 实际 Harness 子进程、本地模型协议、恢复/故障/权限）、`pnpm check:deepseek-consumer`（干净已提交源码的固定 tarball 消费）、`pnpm smoke:deepseek`（显式配置 `DEEPSEEK_API_KEY` 或 `RSS_DEEPSEEK_KEY_FILE`）。后者默认官方端点，也接受显式 DEEPSEEK_BASE_URL/DEEPSEEK_MODEL；冻结配置并按实际 origin 记录 official/configured/local_fixture；`endpointSha256` 摘要实际使用的完整规范化 URL（含路径），不输出明文地址，也不声明已验证后端身份。缺密钥失败，不静默降级为 fixture。

[来源与证据边界](https://dev.azure.com/shengming0923/rss/_git/rss-mdm-agent?path=/docs/reference/deepseek-harness.md&version=GC80f8efc2048de118ad055dc5f0d6006edf4c42ef)。已执行平台与源码/lock/artifact 身份以 PR 及 `.local-ci-runs` 实际结果为准；不声称已经发布 registry 包或完成 Windows/企业平台 T3。

可选 `onDiagnostic` 接收封闭的 stage/reason 和 generation，用于区分配置、依赖、恢复、IPC 和清理故障；不返回原始异常、路径、提示词或凭据。诊断回调抛错不会改变协议行为。准入校验必需 fiber 的 ACTIVE 状态及 create/restore 的 Agent 状态；必需 fiber/配置或工具 guard 漂移使原 incarnation 永久失效。

私有 IPC 的固定 operation 同时绑定请求/响应类型，并在两端校验 envelope；预算耗尽、传输故障和原生输入拒绝分开分类。有效回答遇到传输故障返回 unavailable/reconcile_first，相同回答的确认可幂等重试。待回答回调最多 32 个；已回答幂等记录另存于有界表，不占用未决容量。正常关闭不报告 process_exit 故障，异常 lost 自动停止并清理子进程。

manifest 仅声明实际源码 import 的直接运行 roots；版本为精确值，Harness 版本由 SessionController root 派生。完整解析闭包及 integrity 由 pnpm-lock.yaml 持有，frozen install 校验其 freshness；启动验证已安装的直接 roots，源码检查防止无 import 能力重新混入 direct dependencies。上游 peer 依赖出现在 lock 中不表示该能力已装配。

公共 `ProviderConfiguration` 仅含可序列化身份与配置；受控工具通过 adapter options 的 `tools` 注入，并将同一个 endpoint 与可信 verifier 作为 `ProviderAdmission` 显式传入 `VerifiedProviderSession.open/restore/fork`。所有命令共用带 attempt 的 `dispatch`；cancel/respond（以及支持的 steer）成功返回 `acknowledged`，不生成模型 outcome。原生核实证据经 `SessionCommit.providerFacts` 原子提交。

独立 tarball 消费使用源码 lock 中的固定外部解析及依赖边，校验部署 lock 后 frozen offline 安装，不从本机 registry metadata 另选传递版本。
