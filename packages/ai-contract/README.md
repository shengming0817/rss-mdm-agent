# AI Runtime V5 公共契约

A01 / #2439，范围基线 `ai-runtime-20260918`。本包拥有产品可靠性 schema 和 TypeScript ports；Rust crate `ai-session-contract` 消费生成绑定。AI Host、AI SQLite 和 adapters 使用 Node/TS；Rust 执行授权、批准、intent、审计和结果权威独立。当前交付为契约、确定性测试替身和共用 conformance，不含真实 Host、数据库、模型或工具隔离证明。

## 真源与版本

[schema/runtime.schema.json](schema/runtime.schema.json) 是唯一产品 wire 声明。`pnpm generate:ai-contract` 通过 typify 0.8.0 / json-schema-to-typescript 16.0.0 生成 Rust/TS；`pnpm check:ai-contract` 验证生成物和 Rust 内嵌 schema 投影零差异。只使用内部引用、闭合对象、常量标签 oneOf、enum 和边界约束；不使用复杂条件、anyOf 或任意外部 schema 解析。标准 ACP/A2UI 保持上游 owner。

V5 直接替换 V4 及更早版本。测试用户、个人连接与配置/凭据修订、产品 Session/provider 阶段和显式历史预览由同一 schema 声明；创建 Session 不启动 provider，命令回执固定接纳阶段。C20 增加配对的 delivery_requested / delivery_recorded 事件与 receipt_recorded 交付状态；持久回执可在模型轮次结束或会话待恢复时独立收敛，只有窄化的 delivery commit 可跨原 generation 更新。`command_accepted` 稳定事件完整携带不可变 Command；旧 `status/accepted` 删除。`accept` 只接收 `eventId`，由同一事务构造事件并提交命令、receipt 与事件，客户端不再补读快照取得用户输入。没有旧 reader、alias、双写、fallback、转换器或历史导入；旧消费者必须整体更新，旧数据库只读拒绝。历史源码与交付证据由 Git/PR 保留。

生成 DTO 只表达数据。外部字节必须经过 `decode(input, limits)`；不要把直接反序列化/类型断言当作验证。所有字符串（含 member name）累计 UTF-8 预算，容器深度最多64，全部 JSON 整数限定安全范围；拒绝重复键、非有限数、非法 Unicode、未知版本/字段。`ContractError.code` 不含正文或动态字段名。`fingerprint(command, limits)` 对已校验命令做 JCS/SHA-256，包含完整输入、expiresAtMs 和 commandId；可信 namespace 单独加入存储唯一键。键顺序不同不产生内容冲突。

## 三个行为 port

`ProviderAgentPort.createSession(configuration, budget)` 原子返回同一 incarnation 的 binding 与 capabilities；其余操作为 submit/cancel/respond/observe/reconcile/close，`resume(binding, configuration, budget)` 是可选接缝，同样原子返回 binding+capabilities，直接替换旧的裸 Binding 返回形状。配置只传显式工作目录、config revision、账号引用、权限选择和宿主 ToolEndpoint，不将秘密写进 wire。ToolEndpoint 返回的是模型工具协议结果，不能签发批准或直连 runner。adapter 自己持有 SDK、进程、模型 transcript 和 native session/run/request ID。

能力分基础会话和受控工具 profile。基础可建立会话、文本多轮、输出/状态/终态和取消；取消能力区分 unsupported/unknown/request_only/terminal_acknowledged。跨进程 resume、steer、fork、子 agent、终端、结构化追问及多模态显式声明；扩展操作由 A01 定义的受控 extension port 承接，公共 V5 input 只接纳文本、取消和回答，不伪造通用多模态载荷。缺少能力不能请求对应操作。

能力声明必须绑定 provider/adapter version、config、账号及 generation；Host 在使用观察事件前核对完整 binding 与命令账本的 native run/request 关联。resume 必须核对 provider/config/account，跨 generation 仅在 across_processes 能力与 provider 成功响应后成立。展示历史不恢复模型上下文。`ProviderConfiguration` 是纯数据，权限为 tools_disabled 或 host_mediated；ToolEndpoint 与可信平台 verifier 通过 parent-only ProviderAdmission 参数单独注入，verifier 不跨 worker IPC。Host 通过 `VerifiedProviderSession.open` 或 `VerifiedProviderSession.restore(port, previousSession, configuration, budget)` 消费原子结果并完成验证；恢复显式消费当前配置，核对原 session ID、新 generation 与 across_processes 能力，重新执行 verifier；已打开 runtime 的准入失败会以独立有界预算关闭，调用方仍须处理/重试未完成的关闭。私有构造和运行时 token 阻止同形对象/JSON 冒充 admission；证据固定完整 binding、capabilities 与 endpoint 对象身份，跨 incarnation 使用须重新验证。verifier 是受信代码边界，具体平台/版本的原生工具旁路证明由 adapter 持有；本包不从字符串或模型声明推导该证明。

`HostPort` 接受可信 ingress 提供的 Caller，提供 createSession、submit/cancel/respond、negotiate、snapshot/subscribe/close。Caller 是组合根的认证前置，不是本包签发的证书；禁止从模型内容、工具参数或 A2UI context 构造它。snapshot 包含稳定事件、命令、交互和 surface 与同一水位；超过明确输出上限返回 limit_exceeded，不静默裁掉历史。subscribe 从 exclusive cursor 回放后接实时；临时 delta 保留 commandId/generation/messageId/text，同一命令的交错消息按 messageId 归并；detach/AbortSignal 只结束订阅。过期 cursor、丢失事件或输出背压必须要求 resync，不伪造终态。

`SessionStore` 提供 create/session/accept/command/commit/rebind/snapshot/events/surface/recovery/delivery/deliveries/retire/pruneRetired/close。所有持久化操作统一返回 Result，分页为 Result<Page<T>>，pruneRetired 为 Result<number>；参数错误返回 invalid_input，后端错误保留 unavailable 和 retry。accept 和 commit 是原子事务接缝；commit 核对 expectedRevision + expectedGeneration、推进 revision、提交稳定事件及必要 delivery，不接纳任意 async 事务回调。普通 commit 不得改变 provider/version、config、account、native session、generation 或 capabilities，运行坐标只能指向已确认 submitted 的活动命令 dispatch，清空须对应命令已 terminal/invalidated 或有明确未提交证据；它不是跨 generation 恢复入口。interaction 的原命令、native run、nativeCallbackId、request、期限和 callbackLifetime 不可重绑。恢复查询和 delivery 查询有页大小与 opaque continuation，不承诺多 worker lease。

## 可靠性与交互

`withinBudget(factory, operation, lifetime?)` 是共享的有界调用封装：将调用方取消、可选实例关闭和 deadline 传到同一个子 `Budget.signal`，并以 watchdog 约束不响应取消的 port。调用结束时清除计时器、解除父 signal 监听并终止子 signal；调用方 signal 不会被子调用结束所取消。Node 24.14.1 的 composite signal 保留问题通过显式监听释放规避，不把 signal 取消当作进程退出或业务副作用回滚证明。

- namespace = tenant + principal + authority + logical session。相同 commandId/相同规范内容返回原 receipt；内容不同返回 content_conflict，不能覆盖旧记录。receipt 只有真实 store commit 后才能称 durable；测试替身的内存接纳不作此承诺。
- accepted 只表示持久接纳；dispatching 表示派发意图已保存；running 需要原生确认；terminal 需要明确模型终态：状态投影必须保留原 dispatch，并与同批、同 command/generation、同 outcome 的 terminal Event 一起提交；accepted 或 unknown 不能凭空变成 completed。dispatching/running 结果不明进入 reconciliation_required，只有带原 attempt 的明确 not_submitted 证据且原重试期限内的 queue_next 才能回到 accepted。普通输入 queue_next；steer 必须精确匹配运行及能力。
- 取消已发送、模型 turn 已停止、进程已退出、业务副作用已停止分别记录。原生 transport/stream 错误不是 terminal；未知提交先核实原运行。`same_command` 重试仅复用同身份/内容；`reconcile_first` 禁止直接再次提交；`never` 不重试。
- 一个命令账本同时持有 inbox、内容绑定、派发意图和 native IDs，不复制第二份 provider 队列。事件日志支持重放，稳定事件先提交后发布；临时 token delta 没有 durable sequence，可丢弃/合并。
- Delivery 只用于需可靠跨服务交付的请求/结果，通过稳定 operationId/eventId、目标与内容摘要关联原事件。摘要固定为 `SHA-256(JCS({event, target}))`，由 `deliveryFingerprint(event, target, limits)` 生成，Store 必须核对完整引用事件和目标。接收方幂等；retry 类别为 receiver_idempotent/reconcile_first/never；未知副作用转核实，不自动重发。待交付记录和引用事件必须一起保留。
- retention.retryWindowMs > 0、receiptWindowMs >= retryWindowMs，精确截止时间写入 receipt。首次接纳拒绝过期 command；receipt 保留期内同内容重复返回既有接纳事实，不能据此重发模型命令。receipt 过期后仍保留键/摘要阻止复用，直到 namespace 退役并清理；达到容量上限应拒绝新接纳。仅所有命令 terminal/invalidated、无 pending 交互且 delivery 已全部 delivered 方可退役，收据过期且 delivery 全结清后才能清理；旧 session ID 永不重建。
- interaction 固定 generation/native run/nativeCallbackId、问题 request、期限及 callbackLifetime。回答接纳与 pending→answered 在同一事务；第二个不同回答返回 already_answered，同命令重复返回 receipt。回调失效为 unavailable，重建 UI 不恢复 callback。仅支持 generation_bound；恢复旧显示内容不会恢复旧回调。

## ACP–A2UI 产品约定

ACP 固定官方 SDK 1.4.0 / schema-v1.21.0。标准 session/new、session/prompt、session/update、session/cancel、session/request_permission 保持上游语义；prompt 的最终响应必须有真实 stopReason，不能提前返回 accepted receipt。无 stopReason 的错误走标准错误路径，不制造成功响应。

产品能力在 capabilities._meta 的 `rss-mdm-agent.ai-runtime` 下协商 contractVersion=5、durableReceipts、cursorAttach，以及可选 A2UI version/catalogId/catalogVersion。`_rss-mdm-agent/submit`、`/snapshot`、`/attach` 和 `/update` 是扩展方法的完整产品前缀约定（代码中的 extension 常量为准），只有协商后使用；未知 request 按 ACP 返回 method-not-found，未知 notification 按上游规则忽略。A01 只冻结约定和 fixtures；实际 transport/协议 service 归 A04。

A2UI 固定 v0.9.1 snapshot，客户端 action、服务端 surface 生命周期、basic catalog 与 common types schema 原样保留在 [upstream](schema/upstream/a2ui/NOTICE.md)。`SurfaceState` 绑定 session/run、surfaceId、surfaceInstanceId、revision、interaction、component/event、catalog/version。create/update/delete 的上游 payload 不改写；产品扩展携带关联 metadata。一次新建 surface 使用新 instance ID，创建 revision 为0，更新/删除以 session revision/generation CAS 为前提严格递增1，身份字段不可重绑；active→deleted 持久化 tombstone 且不可复活，同时使 pending interaction unavailable。snapshotPage 与事件恢复这些关联和实际内容；完整有界 messages 与 surface 稳定事件同批发布，内容校验由公共 validateSurface 完成，A04 在发布和渲染前消费。

`resolveSurfaceAction(store, caller, metadata, standard, limits)` 从 Store 读取当前 surface 后校验关联与 Caller，返回的 context 仍是未经授权的回答数据；名称、timestamp、context 中的 actor/approved 均无批准权。metadata 中的 commandId 由产品客户端为该次回答生成并在重试时复用；它不是权限凭据。返回的 surface instance/revision 必须放入 respond input.surface；Store 在回答接纳事务中再核对 active 状态与 revision，关联 surface 的 interaction 不允许省略该字段。回答单次消费由 Host/Store 保证，解析与提交之间的删除/更新也会拒绝旧回答。无 A2UI 客户端保留文本、工具状态和标准权限交互；专有结构交互明确不支持，不能自动批准或用普通消息替代回答。

## 消费与验收

```sh
pnpm build:ai-contract
pnpm test:ai-contract
pnpm check:ai-contract
pnpm check:ai-consumer
cargo test -p ai-session-contract --locked
```

Node 验证基线24.14.1 / pnpm11.4.0。`/transitions` 导出同步纯状态转换，MemorySessionStore 与真实存储 adapter 复用同一规则；不包含测试依赖或事务内外部 await。主导出是生成类型、编解码、ports 和协议关联函数；`/testing` 导出 FakeHost、MemorySessionStore、ScriptedProvider、共用 fixtures、runHostConformance、runStoreConformance 与 runProviderConformance。Host 工厂需提供零时钟、无终态的确定性 provider；Provider 工厂需提供 submitted/unknown 两种可控上游脚本，并结束观察流。Host/Store 的 close 立即停止新准入、结束订阅/worker，再按 provider→store 顺序释放资源；关闭幂等，失败可用新 budget 重试，不能把退出请求当作资源已释放。套件对每个工厂、异步操作、迭代器 next/return 和 close 使用独立 budget 与 watchdog；Provider 套件第三参为 `() => Budget`。忽略 AbortSignal 的 Promise 也会有界退出并进入清理，主体和清理错误聚合保留。watchdog 不宣称能抢占阻塞事件循环的同步代码，也不把超时视为真实进程已停止。共享场景用于后续真实 Host/adapter/stores 的同一验收入口；真实 SQLite 故障/崩溃、模型输出与工具旁路另外提供证据。

测试替身无 provider worker、进程调度、数据库或恢复后台任务。FakeHost 的固定窗口和1024条 snapshot 上限只是确定性测试配置。Rust 独立 consumer 和 TS tarball consumer 实际运行公共 API；不依赖 Tauri、相邻仓、原 workspace 的隐式依赖或 Rust 生成工具运行时。

## A01 回调契约替换（#2406）

`Interaction.nativeRequestId` 直接替换为必填 `nativeCallbackId`，同时必填 `request`；旧记录、旧字段和双字段输入均拒绝，不提供迁移、alias 或双读。`Binding` / dispatch 的 `nativeRequestId` 仅表示父 prompt/query 请求，原命令通过 `Interaction.commandId` 关联；一个父请求可有多个独立 callback。callback ID 在 namespace + generation 内唯一，不能换 interactionId 重复消费。

`ProviderObservation` 新增 `type: "interaction"`，`interaction: ProviderInteraction` 从同一生成 wire 类型选取 callback ID、产品 interaction ID、期限、lifetime 和问题载荷。Host 先验证完整 binding 与 command dispatch，从可信会话补 namespace/generation/nativeRunId，再原子提交 pending Interaction 与同 ID/command/generation、内容相同的 interaction 事件。首次 pending 必须携带与 Interaction 相等的 request、expiresAtMs 和 callbackLifetime；answered 必须携带相同 responseCommandId；其它状态事件不得携带 request。所有交互状态变化都必须有同批匹配事件。创建只接受已确认提交的活动命令，缺一侧或重复 pending 均拒绝。`request` 是有预算的、不可信 provider JSON，不是执行工具提案，也不是第二套 UI schema；A04 拥有展示适配。

`dispatch(binding, command, attempt, budget)` 统一派发响应与其他控制命令。Host 负责可信 Caller 与 Store 的单次接纳；adapter 通过 binding + interactionId 定位私有活回调并核验 generation/期限，不能从客户端提交的 ID 构造回调。回答不形成执行批准，执行工具与追问回调隔离。显示历史保留 request，但 generation_bound callback 丢失后必须 unavailable。

共同 `runStoreConformance` 包含一个父请求的多个 callback、反序回答、callback 别名拒绝及 pending 记录/事件的原子提交。Claude/Codex/其它 adapter 直接消费此接口；本包没有 SDK 依赖，也没有新增 Host worker。

回调身份参考固定发布包 [Claude Agent SDK 0.3.277 sdk.d.ts](https://unpkg.com/@anthropic-ai/claude-agent-sdk@0.3.277/sdk.d.ts) 的 `CanUseTool`：每次回调独立携带 `requestId` 和 `toolUseID`，不能与父 prompt 身份合并。此引用不形成运行依赖或 SDK 互操作证明。

Interaction 的必填 `category: "question"` 仅允许普通用户追问；权限 callback 不属于普通 respond 生命周期，进入 ToolEndpoint/verifier 或拒绝。Provider 的 pending 发布只能使用专用 interaction observation。wire schema 按状态闭合：pending 必带 request，answered/expired/unavailable 禁带 request；旧格式直接拒绝，TS/Rust 由同一 schema 生成。

A04 同 PR 的直接契约替换、分页/列表、明确终态与浏览器入口见[ACP–A2UI 开发](../../docs/guides/ai-access-development.md)。协商字段和全部产品扩展 DTO 从同一 schema 生成；通用包入口没有 Node fs/crypto，生成期编译静态校验器，运行期使用固定 noble 摘要。旧单次 snapshot API 不再提供。

A2UI version/catalogId/catalogVersion 的声明在 runtime schema 的 A2uiNegotiation，生成的冻结 `interactionCatalog` 为运行时唯一入口；`selectNegotiation` 在双端限制 selection≤offer。Memory store 完成且未签发 continuation 的单页读视图立即释放；已签发 token 的视图保留至 TTL，支持原 token 的重复读取。

## A02 前置恢复契约（#2440）

`DispatchAttempt` 必填 attemptId、originGeneration、observerGeneration 和 nativeSessionId。Host 必须先保存 certainty=intent 再调用 `dispatch(binding, command, attempt, budget)`；原始身份固定，native run/request/correlationId 只可首次补入。ProviderObservation 必须携带原 attemptId，Host 同时核对当前 observer 与完整 binding。unknown 的 correlationId 持久化，stream 中断或进程退出不能清除派发事实。

`CommandRecord` 按状态闭合：accepted 无 dispatch；dispatching/running/reconciliation_required 有 dispatch；terminal 同时有 dispatch/outcome；invalidated 只有本地 failure，不能伪造模型终态。普通 commit 不创建命令，接纳只经 accept。状态改变和派发坐标补入必须携带匹配的稳定事件。

`VerifiedProviderSession.restore` 消费 provider.resume 的原子 binding/capabilities，并重新核验配置、账号、provider/adapter 版本、原 native session 和受控工具 verifier。`SessionStore.rebind` 只消费这份不可序列化的恢复证据，以 revision/generation CAS 原子切换会话、更新未决 attempt 的 observerGeneration、保留 originGeneration 与全部原生坐标、转为 reconciliation_required。已接纳但未派发的旧代回答/取消/steer 失效；已派发的控制命令保留待核实；queue_next 仍保留。旧 generation 永不复用。

同一 rebind 原子使 pending interaction unavailable、active surface invalidated，并追加 session_rebound、状态、交互及 status=invalidated 的完整 surface 事件。历史问题与 A2UI payload 留在事件日志；失效不是上游 deleteSurface。create/update/delete 的原样 payload 必须与 surface revision、关联行和稳定水位同事务提交；删除还需同批提交交互失效及其事件。

`SessionCommit.providerFacts` 处理 running/terminal/not_submitted/unknown；不存在第二个恢复提交 API 或 provider 队列。证据必须指向原 attempt 和当前 observer，原生坐标不可覆盖。unknown 保持阻断；not_submitted 将完整旧 attempt（含 correlationId）写入 reconciled 事件，且仅原期限内 queue_next 可再次接纳派发，新派发必须使用新 attemptId。已过期或绑定旧代的控制输入进入 invalidated。重试不扩大原 receipt 窗口。

共用 conformance 覆盖多代恢复、拒绝结构化假证据、旧 observer / 旧 attempt 回调、正反向核实证据、旧控制命令与 surface 失效、水位连续回放。真实进程故障与 SQLite 事务由 A02 单独验证。

恢复证据同时绑定原 Namespace 与 Binding，禁止相同原生 binding 的跨租户重放。每个 ProviderAgentPort 实例只供一次 open/restore 准入；并发或重复准入拒绝，不关闭已成功的实例。失败路径使用新预算关闭所消费的实例；`AdmissionResult.cleanupError` 保留未完成的清理结果，调用方仍持有 port 并可重试 close。adapter 的 close 必须清理迟于 abort 完成的初始化工作；本包 watchdog 只能限定等待，不能抢占外部进程。

provider 通用 event 使用 `ProviderEventBody` 白名单，只含文本、工具提案/结果、取消观察、错误、surface 和明确终态；内部派发、核实、会话/回调/本地失效事件由 Host/Store 产生。新 attempt 提交时必须再次提供 nowMs 并核对原 deadline，不以先前窗口内的 not_submitted 核实替代当前检查。

`deliveries` 返回到期的 pending 与 reconciliation_required 记录，调用方必须先检查 status/retry；查询不代表领取或允许重发。仅全部 delivered 才允许 retire。`StoreCursor` 是1–2048字符的 opaque continuation，与 wire Id 分离，调用方仅原样回传给同一 adapter。

外部复核后的契约收口：ProviderConfiguration 必须由 composition root 提供完整 namespace；准入时复制它并绑定该 provider 实例。`VerifiedProviderSession.reconcile(session, record, budget)` 是统一 provider 事实入口之一，实际调用所准入的 port，绑定当前完整 binding、namespace 和原始 CommandRecord。返回的 `VerifiedProviderFact` 通过模块私有 WeakMap 校验，公开 observation 只返回副本；结构转换、JSON 复制或改写原始记录均不能生成有效证据。SessionCommit.providerFacts 只消费这类凭证，不接受原始 provider 结果。原始 provider 方法仍返回普通观察数据；数据本身不是重试许可。

Binding 新增必填 workspaceId：`workspaceIdentity(workingDirectory)` 对规范化绝对路径计算 SHA-256，restore 必须与原 binding 相同，实际传给 adapter 的路径在外部 await 前规范化固定。它是逻辑目录身份，不是 symlink、挂载或操作系统 containment 证明；平台隔离仍由 adapter/verifier 持有。旧 binding 无此字段直接拒绝。callbackLifetime 只保留 generation_bound；未实现的 provider_resumable 已删除，无兼容分支。

命令 terminal/invalidated 必须在同一提交结束该命令所有 pending Interaction 并使 active SurfaceState 失效，同时附带对应稳定事件；缺任一状态或事件则整批拒绝。pending→expired 必须传有效 nowMs，且严格晚于包含端点的 expiresAtMs。rebind 的事件 seed 先经 schema-owned isId 检查；codec、transition 与 adapter 共用该原语。

Rust 生成时将同一 schema 的 const 等价投影为单值 enum，以补足 typify 0.8 对 const 的忽略；Event variant 名由原始判别字段稳定生成。wire schema 仍是唯一来源；Rust 变体匹配与独立 consumer 同时验证 error/invalidated 和完整 surface 状态的真实类型。

SessionState 是 adapter 持有的完整持久化状态，包括全部历史 generations。createState 仅创建 revision/sequence 为0的全新会话；SnapshotPage 是显示读模型，不能拿来重建 persistence state。共享恢复检查涵盖两种调用顺序的并发 rebind/旧代 commit；SQLite adapter 另外以真实重启验证 generation 历史。

Claude adapter 的合并集成同样使用显式 DispatchAttempt 和完整 CommandRecord。ProviderObservation 新增两类原生事实：submitted 表示该 attempt 获得原生接纳确认；interaction_unavailable 表示活 callback 已失效。它们携带当前 binding/commandId/attemptId，Host 验证后原子更新账本和稳定事件，adapter 不直接发布 Host 生命周期事件。共享 Provider conformance 的迟到 resume 场景从另一个已准入并关闭的实例取得真实 prior binding，避免用无效测试版本绕过实际恢复路径。

集成入口：浏览器使用主入口的 codec、wire 与协议；Node Host 从 `@rss-mdm-agent/ai-contract/session` 导入 VerifiedProviderSession/workspaceIdentity，从 `/transitions` 消费持久化状态规则。内存和 SQLite 共用 `/read-views` 的 snapshotPage/listSessions 同水位分页；续页绑定 scope、limit 和不可变视图，30秒失效，重启后返回 cursor_expired，单实例最多128份视图/16MiB保留内容。客户端重新抓取快照后按持久 cursor 接续，分页缓存不授予回调权限。

A03 收口：ProviderAgentPort 仅保留统一 `dispatch`，原生提交确认保持 dispatching，实际运行观察才进入 running。控制命令 acknowledged 和本地 queued_cancelled / cancelled 是闭合终态，不使用模型 Outcome。Capabilities 不再声明 queue；持久 FIFO 属于 Host。`VerifiedProviderFact` 覆盖 dispatch / observe / reconcile，并在当前 revision/generation CAS 时校验原 attempt。`SessionStore.recoverUnavailable` 原子冻结恢复不可用的旧绑定及回调，`Session.status=recovery_required` 由同一列表/快照展示。OS 启动 fence 由 Host 独立 `WorkerLaunchFenceStore` 持有，公共 `SessionStore` 不含 PID/PGID/artifact。详见 [Host](../ai-host/README.md)。

## 原生 thread 与 turn 的多命令绑定（#2405）

Binding 和 DispatchAttempt 增加可选 nativeThreadId；拥有独立 thread 的 provider 必须同时持久化原生 session/thread，不得折叠或互相推导。它属于跨 generation 不可变的 providerIdentity；恢复、reconcile、观察投影和 SQLite 均校验。原生模型只有 session 的 provider 不填写 thread，禁止为 Codex 缺失 thread 的记录伪造兼容值。

同一原生 turn 可以接收一个普通 prompt 和多个 steer，每条命令保留独立 attempt/nativeRequestId。任何未决已派发 prompt（含 unknown）都会阻止下一条普通 prompt；steer 只能定位当前已确认普通 prompt 的活动 run，intent 即携带目标 nativeRunId。输出按实际命令坐标核验，不能把 session.binding 当前 request 当成同 turn 唯一 request。run/request 只有在该 turn 的所有已派发 prompt 结束后才可整体清除；未确认 steer 不因普通 prompt 终态而自动结束，必须核实原 attempt。ACP cancel 按 native session/thread/run 去重，优先选普通 prompt 作为中断目标。

## Provider 扩展与诊断

`ProviderInstance` 明确分成 `agent: ProviderAgentPort`、`extensions.fork?: ProviderForkPort` 和 `diagnostics: ProviderDiagnosticsPort`。正式方法不返回 provider 生成的实验 DTO；history 由 adapter 内部恢复算法持有。diagnostics 只包含封闭种类与丢弃计数，不承接可靠业务事件。

Host 先持有 child instance，再用已经准入的 `parentSession.fork(child, throughTurnId, configuration, budget)` 执行 A01 共同准入。源 namespace/绑定取自名义 parent proof，不接收调用方伪造来源。该操作验证权限域、账号、配置、workspace、新原生身份及 child 的受控工具 verifier；失败消费并关闭 child，保留 unknown 与 cleanupError，绝不重试 native 创建。adapter 的 extension 只返回原生事实，不反向构造或准入子会话。Host/A03 持有创建意图和结果的持久化、未确认实例清理与客户端发布；现有基础 ProviderAgentPort 消费者可继续只消费 agent。
