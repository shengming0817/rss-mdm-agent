# AI Session Host

`@rss-mdm-agent/ai-host` 实现 A03（#2441），消费 A01 契约和 A02 Store，并实现 A04 的 `HostPort`。Node Host 是 AI 数据库的唯一写入者；每个 provider incarnation 在独立 Node worker 中加载 SDK。Rust 执行授权、批准和业务 journal 不在本包。

```ts
const result = await createHost({
  store,
  launchFences: store, // SQLite implements the separate WorkerLaunchFenceStore port.
  resolve: async (caller, options, namespace, budget) => ({
    configuration: { namespace, ...providerSettings },
    artifact: trustedWorkerModuleUrl,
    // controlled_tools 时由组合根提供实际 endpoint 和平台 verifier。
    admission,
  }),
});
```

`artifact` 是可信组合根选定的 `file:` 模块，导出 `createProvider: WorkerFactory`。它只在持久 launch fence 登记后加载。配置是纯数据；parent 独占 verifier，仅在准入验证与 Session 持久化成功后开放 worker 的反向 ToolEndpoint RPC。工具提案不携带身份或执行批准，实际调用者来自 `resolve` 的可信 Caller。调用者必须检查所有 `Result`，包括可重试的 `close`。

`SessionStore` 只承载公共会话语义；`WorkerLaunchFenceStore` 及其 artifact/PID/PGID 校验由 `ai-host/launch-fence` 持有。SQLite 同时实现两个窄 port，组合根分别注入，不向 A01/MemoryStore 增加 OS 接口。

Host 默认普通队列上限 64，控制待处理上限 64，worker 总数 8，每 provider/account 2；控制优先 burst 为 8，provider 操作预算 30 秒。`queueLimit`、`workerLimit`、`accountWorkerLimit`、`operationTimeoutMs` 可配置，无效配置通过 `createHost` 返回 `invalid_input`；控制队列上限与 burst 固定。持久 `CommandRecord` 是唯一队列，内存仅持有 mailbox、运行任务和短时重试截止。长时间模型观察不占用 mailbox；健康 worker 的控制派发由测试验证在 1 秒内完成。

`accepted` 在调用 provider 前持久化，随后持久化 intent。原生排队确认只填入 submitted certainty，仍为 `dispatching`；实际运行观察才进入 `running`。控制命令进入 `acknowledged`，本地排队取消进入 `cancelled`，均不产生模型终态。unknown 始终保留原 attempt / correlation，禁止自动重发。只有核实的 not_submitted 能按原 receipt 窗口重新排队。

`snapshotPage`、`listSessions`、`resume`、`subscribe` 使用完整 Caller namespace。detach 不停止会话。稳定事件先提交再广播；每个订阅者最多 256 项 / 1 MiB，溢出发送 `resync_required` 后断开。私有 IPC 每帧最多 256 KiB，控制、模型输出和工具 RPC 使用独立管道。

`createHost` 先扫描启动 fence，再恢复未决工作。registered group 非空或不可核实时冻结会话；新 Host 不凭保存的 PID 发信号或接管进程。恢复失败持久化为 `Session.status=recovery_required`，保留旧身份与普通队列，失效旧回调和未派发控制。外部清理后可再次 `resume`。无未决工作的会话按需恢复；launch fence 独立扫描，idle 会话同样冻结。尚未创建 Session 的 orphan fence 在后续接纳时复查，只有确认空组才回收，不引入轮询 heartbeat。

`close` 停止接纳并等待已进入的启动/恢复操作退出，保留排队工作，有界取消正在运行的任务，记录未决状态，再关闭 worker 和 Store。成功要求真实 root exit 与进程组为空；Abort、信号发送和 adapter 的布尔声明都不能单独证明进程退出。Host 失联时 worker 失效反向工具桥并清理其组。整个 close 共用绝对 deadline，包括 Store、mailbox、admission 与 fence 释放；预算耗尽仍同步失效 IPC 并升级终止当前句柄，保留未完成清理供 fresh budget 重试。失败恢复立即隔离 runtime，Store 失败进入 `recovery` diagnostic；不能持久化时保留 blocked runtime/fence。无法证明空组时返回可重试失败。

验证入口：`pnpm test:ai-host`、`pnpm check:ai-host-consumer`。测试区分真实 SQLite/OS 进程、固定模型 HTTP transport + 实际 Claude SDK，以及外部模型实测；前两者不能替代后者。macOS arm64 是本次运行包的实际验证平台。桌面 Tauri 装配继续由 #2413 持有。

参见[本地应用](../../apps/ai-host/README.md)、[设计](../../docs/architecture/ai-host.md)及[来源](../../docs/reference/ai-host.md)。
