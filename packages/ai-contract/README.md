# AI Runtime V2 公共契约

A01 / #2439，范围基线 `ai-runtime-20260918`。本包拥有产品可靠性 schema 和 TypeScript ports；Rust crate `ai-session-contract` 消费生成绑定。AI Host、AI SQLite 和 adapters 使用 Node/TS；Rust 执行授权、批准、intent、审计和结果权威独立。当前交付为契约、确定性测试替身和共用 conformance，不含真实 Host、数据库、模型或工具隔离证明。

## 真源与版本

[schema/runtime.schema.json](schema/runtime.schema.json) 是唯一产品 wire 声明。`pnpm generate:ai-contract` 通过 typify 0.8.0 / json-schema-to-typescript 16.0.0 生成 Rust/TS；`pnpm check:ai-contract` 验证生成物和 Rust 内嵌 schema 投影零差异。只使用内部引用、闭合对象、常量标签 oneOf、enum 和边界约束；不使用复杂条件、anyOf 或任意外部 schema 解析。标准 ACP/A2UI 保持上游 owner。

V2 一次替换 C02 的 V1。没有 V1 reader、alias、双写、fallback、转换器或历史数据导入。旧 Message/Proposal 的展示和不可信语义在 V2 EventBody 延续；旧命令缺少 commandId、可信 namespace 和派发账本，不可重建派发资格。升级消费者应重新建立会话。#2395 的历史源码、tests 和交付证据由 Git/PR 保存。

生成 DTO 只表达数据。外部字节必须经过 `decode(input, limits)`；不要把直接反序列化/类型断言当作验证。所有字符串（含 member name）累计 UTF-8 预算，容器深度最多64，全部 JSON 整数限定安全范围；拒绝重复键、非有限数、非法 Unicode、未知版本/字段。`ContractError.code` 不含正文或动态字段名。`fingerprint(command, limits)` 对已校验命令做 JCS/SHA-256，包含完整输入、expiresAtMs 和 commandId；可信 namespace 单独加入存储唯一键。键顺序不同不产生内容冲突。

## 三个行为 port

`ProviderAgentPort.createSession(configuration, budget)` 原子返回同一 incarnation 的 binding 与 capabilities；其余操作为 submit/cancel/respond/observe/reconcile/close，resume 是可选接缝。配置只传显式工作目录、config revision、账号引用、权限选择和宿主 ToolEndpoint，不将秘密写进 wire。ToolEndpoint 返回的是模型工具协议结果，不能签发批准或直连 runner。adapter 自己持有 SDK、进程、模型 transcript 和 native session/run/request ID。

能力分基础会话和受控工具 profile。基础可建立会话、文本多轮、输出/状态/终态和取消；取消能力区分 unsupported/unknown/request_only/terminal_acknowledged。跨进程 resume、steer、fork、子 agent、终端、结构化追问及多模态显式声明；扩展操作由具体 adapter 扩展接口承接，公共 V2 input 只接纳文本、取消和回答，不伪造通用多模态载荷。缺少能力不能请求对应操作。

能力声明必须绑定 provider/adapter version、config、账号及 generation；Host 在使用观察事件前核对完整 binding 与命令账本的 native run/request 关联。resume 必须核对 provider/config/account，跨 generation 仅在 across_processes 能力与 provider 成功响应后成立。展示历史不恢复模型上下文。`ProviderConfiguration` 是闭合权限联合：tools_disabled 禁止 endpoint/verifier；host_mediated 必须同时提供 ToolEndpoint 与组合根注入的可信平台 verifier。Host 通过 `VerifiedProviderSession.open` 消费原子结果并完成验证，私有构造和运行时 token 阻止同形对象/JSON 冒充 admission；证据固定完整 binding、capabilities 与 endpoint 对象身份，跨 incarnation 使用须重新验证。verifier 是受信代码边界，具体平台/版本的原生工具旁路证明由 adapter 持有；本包不从字符串或模型声明推导该证明。

`HostPort` 接受可信 ingress 提供的 Caller，提供 createSession、submit/cancel/respond、negotiate、snapshot/subscribe/close。Caller 是组合根的认证前置，不是本包签发的证书；禁止从模型内容、工具参数或 A2UI context 构造它。snapshot 包含稳定事件、命令、交互和 surface 与同一水位；超过明确输出上限返回 limit_exceeded，不静默裁掉历史。subscribe 从 exclusive cursor 回放后接实时；临时 delta 保留 commandId/generation/messageId/text，同一命令的交错消息按 messageId 归并；detach/AbortSignal 只结束订阅。过期 cursor、丢失事件或输出背压必须要求 resync，不伪造终态。

`SessionStore` 提供 create/session/accept/command/commit/snapshot/events/surface/recovery/deliveries/retire/pruneRetired/close。所有持久化操作统一返回 Result，分页为 Result<Page<T>>，pruneRetired 为 Result<number>；参数错误返回 invalid_input，后端错误保留 unavailable 和 retry。accept 和 commit 是原子事务接缝；commit 核对 expectedRevision + expectedGeneration、推进 revision、提交稳定事件及必要 delivery，不接纳任意 async 事务回调。普通 commit 不得改变 provider/version、config、account、native session、generation 或 capabilities，运行坐标只能指向已确认 submitted 的活动命令 dispatch，清空须对应命令已 terminal；它不是跨 generation 恢复入口。interaction 的原命令、native run、nativeCallbackId、request、期限和 callbackLifetime 不可重绑。恢复查询和 delivery 查询有页大小与 opaque continuation，不承诺多 worker lease。

## 可靠性与交互

- namespace = tenant + principal + authority + logical session。相同 commandId/相同规范内容返回原 receipt；内容不同返回 content_conflict，不能覆盖旧记录。receipt 只有真实 store commit 后才能称 durable；测试替身的内存接纳不作此承诺。
- accepted 只表示持久接纳；dispatching 表示派发意图已保存；running 需要原生确认；terminal 需要明确模型终态：状态投影必须保留原 dispatch，并与同批、同 command/generation、同 outcome 的 terminal Event 一起提交；accepted 或 unknown 不能凭空变成 completed。dispatching/running 结果不明进入 reconciliation_required，不能退回 accepted 自动重发。普通输入 queue_next；steer 必须精确匹配运行及能力。
- 取消已发送、模型 turn 已停止、进程已退出、业务副作用已停止分别记录。原生 transport/stream 错误不是 terminal；未知提交先核实原运行。`same_command` 重试仅复用同身份/内容；`reconcile_first` 禁止直接再次提交；`never` 不重试。
- 一个命令账本同时持有 inbox、内容绑定、派发意图和 native IDs，不复制第二份 provider 队列。事件日志支持重放，稳定事件先提交后发布；临时 token delta 没有 durable sequence，可丢弃/合并。
- Delivery 只用于需可靠跨服务交付的请求/结果，通过稳定 operationId/eventId、目标与内容摘要关联原事件。摘要固定为 `SHA-256(JCS({event, target}))`，由 `deliveryFingerprint(event, target, limits)` 生成，Store 必须核对完整引用事件和目标。接收方幂等；retry 类别为 receiver_idempotent/reconcile_first/never；未知副作用转核实，不自动重发。待交付记录和引用事件必须一起保留。
- retention.retryWindowMs > 0、receiptWindowMs >= retryWindowMs，精确截止时间写入 receipt。首次接纳拒绝过期 command；receipt 保留期内同内容重复返回既有接纳事实，不能据此重发模型命令。receipt 过期后仍保留键/摘要阻止复用，直到 namespace 退役并清理；达到容量上限应拒绝新接纳。仅所有命令 terminal、无 pending 交互方可退役，收据过期且 delivery 全结清后才能清理；旧 session ID 永不重建。
- interaction 固定 generation/native run/nativeCallbackId、问题 request、期限及 callbackLifetime。回答接纳与 pending→answered 在同一事务；第二个不同回答返回 already_answered，同命令重复返回 receipt。回调失效为 unavailable，重建 UI 不恢复 callback。provider_resumable 仍须 adapter 证明真实恢复；不能从序列化 capability 直接推断。

## ACP–A2UI 产品约定

ACP 固定官方 SDK 1.4.0 / schema-v1.21.0。标准 session/new、session/prompt、session/update、session/cancel、session/request_permission 保持上游语义；prompt 的最终响应必须有真实 stopReason，不能提前返回 accepted receipt。无 stopReason 的错误走标准错误路径，不制造成功响应。

产品能力在 capabilities._meta 的 `rss-mdm-agent.ai-runtime` 下协商 contractVersion=2、durableReceipts、cursorAttach，以及可选 A2UI version/catalogId/catalogVersion。`_rss-mdm-agent/submit`、`/snapshot`、`/attach` 和 `/update` 是扩展方法的完整产品前缀约定（代码中的 extension 常量为准），只有协商后使用；未知 request 按 ACP 返回 method-not-found，未知 notification 按上游规则忽略。A01 只冻结约定和 fixtures；实际 transport/协议 service 归 A04。

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

Node 验证基线24.14.1 / pnpm11.4.0。主导出是生成类型、编解码、ports 和协议关联函数；`/testing` 导出 FakeHost、MemorySessionStore、ScriptedProvider、共用 fixtures、runHostConformance、runStoreConformance 与 runProviderConformance。Host 工厂需提供零时钟、无终态的确定性 provider；Provider 工厂需提供 submitted/unknown 两种可控上游脚本，并结束观察流。Host/Store 的 close 立即停止新准入、结束订阅/worker，再按 provider→store 顺序释放资源；关闭幂等，失败可用新 budget 重试，不能把退出请求当作资源已释放。套件对每个工厂、异步操作、迭代器 next/return 和 close 使用独立 budget 与 watchdog；Provider 套件第三参为 `() => Budget`。忽略 AbortSignal 的 Promise 也会有界退出并进入清理，主体和清理错误聚合保留。watchdog 不宣称能抢占阻塞事件循环的同步代码，也不把超时视为真实进程已停止。共享场景用于后续真实 Host/adapter/stores 的同一验收入口；真实 SQLite 故障/崩溃、模型输出与工具旁路另外提供证据。

测试替身无 provider worker、进程调度、数据库或恢复后台任务。FakeHost 的固定窗口和1024条 snapshot 上限只是确定性测试配置。Rust 独立 consumer 和 TS tarball consumer 实际运行公共 API；不依赖 Tauri、相邻仓、原 workspace 的隐式依赖或 Rust 生成工具运行时。

## A01 回调契约替换（#2406）

`Interaction.nativeRequestId` 直接替换为必填 `nativeCallbackId`，同时必填 `request`；旧记录、旧字段和双字段输入均拒绝，不提供迁移、alias 或双读。`Binding` / dispatch 的 `nativeRequestId` 仅表示父 prompt/query 请求，原命令通过 `Interaction.commandId` 关联；一个父请求可有多个独立 callback。callback ID 在 namespace + generation 内唯一，不能换 interactionId 重复消费。

`ProviderObservation` 新增 `type: "interaction"`，`interaction: ProviderInteraction` 从同一生成 wire 类型选取 callback ID、产品 interaction ID、期限、lifetime 和问题载荷。Host 先验证完整 binding 与 command dispatch，从可信会话补 namespace/generation/nativeRunId，再原子提交 pending Interaction 与同 ID/command/generation、内容相同的 interaction 事件。首次 pending 必须携带与 Interaction 相等的 request、expiresAtMs 和 callbackLifetime；answered 必须携带相同 responseCommandId；其它状态事件不得携带 request。所有交互状态变化都必须有同批匹配事件。创建只接受已确认提交的活动命令，缺一侧或重复 pending 均拒绝。`request` 是有预算的、不可信 provider JSON，不是执行工具提案，也不是第二套 UI schema；A04 拥有展示适配。

`respond(binding, command, budget)` 签名不变。Host 负责可信 Caller 与 Store 的单次接纳；adapter 通过 binding + interactionId 定位私有活回调并核验 generation/期限，不能从客户端提交的 ID 构造回调。回答不形成执行批准，执行工具与追问回调隔离。显示历史保留 request，但 generation_bound callback 丢失后必须 unavailable。

共同 `runStoreConformance` 包含一个父请求的多个 callback、反序回答、callback 别名拒绝及 pending 记录/事件的原子提交。Claude/Codex/其它 adapter 直接消费此接口；本包没有 SDK 依赖，也没有新增 Host worker。

回调身份参考固定发布包 [Claude Agent SDK 0.3.277 sdk.d.ts](https://unpkg.com/@anthropic-ai/claude-agent-sdk@0.3.277/sdk.d.ts) 的 `CanUseTool`：每次回调独立携带 `requestId` 和 `toolUseID`，不能与父 prompt 身份合并。此引用不形成运行依赖或 SDK 互操作证明。

Interaction 的必填 `category: "question"` 仅允许普通用户追问；权限 callback 不属于普通 respond 生命周期，进入 ToolEndpoint/verifier 或拒绝。Provider 的 pending 发布只能使用专用 interaction observation。wire schema 按状态闭合：pending 必带 request，answered/expired/unavailable 禁带 request；旧格式直接拒绝，TS/Rust 由同一 schema 生成。

A04 同 PR 的直接契约替换、分页/列表、明确终态与浏览器入口见[ACP–A2UI 开发](../../docs/guides/ai-access-development.md)。协商字段和全部产品扩展 DTO 从同一 schema 生成；通用包入口没有 Node fs/crypto，生成期编译静态校验器，运行期使用固定 noble 摘要。旧单次 snapshot API 不再提供。

A2UI version/catalogId/catalogVersion 的声明在 runtime schema 的 A2uiNegotiation，生成的冻结 `interactionCatalog` 为运行时唯一入口；`selectNegotiation` 在双端限制 selection≤offer。Memory store 完成且未签发 continuation 的单页读视图立即释放；已签发 token 的视图保留至 TTL，支持原 token 的重复读取。
