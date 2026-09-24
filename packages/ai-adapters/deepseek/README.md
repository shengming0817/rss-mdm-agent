# DeepSeek Harness adapter

单包 Node.js / TypeScript 适配器，公开 `createDeepSeekAdapter(options): ProviderAgentPort`。消费 A01 的 `DispatchAttempt` 和 `VerifiedProviderSession`；每个 port 只允许一次 open/restore，独占一个 Node 子进程与原生 Context。公共配置、namespace、账号引用和存储目录由可信组合根提供。协议为 chat-completions；可信 resolver 必须显式提供 HTTPS API endpoint（loopback 测试可用 HTTP），smoke 默认使用官方 `https://api.deepseek.com`。密钥由 resolver 返回，仅通过私有 IPC 进入进程内存。

配置与工厂接口见 [src](src/)，版本由 [manifest](package.json) 和 lock 持有。

`workspaceIdentity` 只表示公共契约中的逻辑工作目录身份。它不证明操作系统隔离。受控模式仍要求消费方提供 `ControlledToolVerifier`，绑定该 incarnation、平台和 `ToolEndpoint`；本包不提供通用批准器或生产平台认证。配置引用、模型、存储根或静态装配改变后不得恢复旧原生 session。

原生 Gateway 提供文本、结构化问题与明确终态；没有终态的断流保留未知。执行、动态插件、委派与任意 MCP 不装配。工具清单变化使当前 incarnation 失效，产品受控准入见[Host](../../../apps/ai-host/README.md)。

恢复必须遵循 A01 流程：

1. 新建 port，`VerifiedProviderSession.restore(port, previousSession, configuration, budget)` 重新准入。
2. 恢复仅装配服务并 `inspect` 原生历史，不调用 follow/resume、不创建 Agent、不请求模型、不修复原生日志。
3. Host 使用恢复凭证 `SessionStore.rebind`，重新读取 Session 与 CommandRecord。
4. `admitted.reconcile(currentSession, record, budget)` 返回私有 `VerifiedProviderFact`，Host 通过 `SessionCommit.providerFacts` 原子提交。
5. 已结算的耐久终态按 exact previous request 只读核实后可继续新命令；缺失历史、inbox 未完成、冷读补出的 `interrupted` 都保持 unknown，持续阻断；本适配器不从日志缺失推导 `not_submitted`。后续显式 dispatch 才在同一已准入 Context 激活原生 Agent。

原 attemptId、originGeneration、correlationId 和已持久化原生坐标保留，observerGeneration 随重绑变化。适配器只发原生事实；交互/surface 的原子失效、合法重试和命令期限由 Host/Store 持有。该 API 不能感知 Host 是否已提交凭证，调用顺序由 Host 执行。

静态服务声明在 `src/assembly.ts`，同时驱动启动和组合摘要；不加载通用 RPC 或 Loader。私有 IPC 固定为 initialize/prompt/inspect/cancel/answer/tool_result/close。原生 Connection 仅为服务注册依赖，不启动网络 listener。临时 Harness home 含其自身 browser-session secret，只有子进程退出且目录清理成功才报告停止。API key 不写入该目录。close 强制终止本 incarnation，未 flush 的原生事实可能丢失，因此未完成命令必须核实。

验证入口：`pnpm test:ai-deepseek`（A01 fixture + 实际 Harness 子进程、本地模型协议、恢复/故障/权限）、`pnpm smoke:deepseek`（显式配置 `DEEPSEEK_API_KEY` 或 `RSS_DEEPSEEK_KEY_FILE`）。后者默认官方端点，也接受显式 DEEPSEEK_BASE_URL/DEEPSEEK_MODEL；冻结配置并按实际 origin 记录 official/configured/local_fixture；`endpointSha256` 摘要实际使用的完整规范化 URL（含路径），不输出明文地址，也不声明已验证后端身份。缺密钥失败，不静默降级为 fixture。

[来源与许可](../../../docs/reference/deepseek-harness.md)。原生进程与本地协议验证不能替代 Windows 或企业平台验收。

可选 `onDiagnostic` 接收封闭的 stage/reason 和 generation，用于区分配置、依赖、恢复、IPC 和清理故障；不返回原始异常、路径、提示词或凭据。诊断回调抛错不会改变协议行为。准入校验必需 fiber 的 ACTIVE 状态及 create/restore 的 Agent 状态；必需 fiber/配置或工具 guard 漂移使原 incarnation 永久失效。

私有 IPC 的固定 operation 同时绑定请求/响应类型，并在两端校验 envelope；预算耗尽、传输故障和原生输入拒绝分开分类。有效回答遇到传输故障返回 unavailable/reconcile_first，相同回答的确认可幂等重试。待回答回调最多 32 个；已回答幂等记录另存于有界表，不占用未决容量。正常关闭不报告 process_exit 故障，异常 lost 自动停止并清理子进程。

manifest 仅声明实际源码 import 的直接运行 roots；版本为精确值，Harness 版本由 SessionController root 派生。完整解析闭包及 integrity 由 pnpm-lock.yaml 持有，frozen install 校验其 freshness；启动验证已安装的直接 roots，源码检查防止无 import 能力重新混入 direct dependencies。上游 peer 依赖出现在 lock 中不表示该能力已装配。

公共 `ProviderConfiguration` 仅含可序列化身份与配置；受控工具通过 adapter options 的 `tools` 注入，并将同一个 endpoint 与可信 verifier 作为 `ProviderAdmission` 显式传入 `VerifiedProviderSession.open/restore/fork`。所有命令共用带 attempt 的 `dispatch`；cancel/respond（以及支持的 steer）成功返回 `acknowledged`，不生成模型 outcome。原生核实证据经 `SessionCommit.providerFacts` 原子提交。
