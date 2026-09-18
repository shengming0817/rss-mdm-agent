# AI Runtime V2 公共契约

A01 / #2439，范围基线 `ai-runtime-20260918`。本包拥有产品可靠性 schema 和 TypeScript ports；Rust crate `ai-session-contract` 消费生成绑定。AI Host、AI SQLite 和 adapters 使用 Node/TS；Rust 执行授权、批准、intent、审计和结果权威独立。当前交付为契约、确定性测试替身和共用 conformance，不含真实 Host、数据库、模型或工具隔离证明。

## 真源与版本

[schema/runtime.schema.json](schema/runtime.schema.json) 是唯一产品 wire 声明。`pnpm generate:ai-contract` 通过 typify 0.8.0 / json-schema-to-typescript 16.0.0 生成 Rust/TS；`pnpm check:ai-contract` 验证生成物和 Rust 内嵌 schema 投影零差异。只使用内部引用、闭合对象、常量标签 oneOf、enum 和边界约束；不使用复杂条件、anyOf 或任意外部 schema 解析。标准 ACP/A2UI 保持上游 owner。

V2 一次替换 C02 的 V1。没有 V1 reader、alias、双写、fallback、转换器或历史数据导入。旧 Message/Proposal 的展示和不可信语义在 V2 EventBody 延续；旧命令缺少 commandId、可信 namespace 和派发账本，不可重建派发资格。升级消费者应重新建立会话。#2395 的历史源码、tests 和交付证据由 Git/PR 保存。

生成 DTO 只表达数据。外部字节必须经过 `decode(input, limits)`；不要把直接反序列化/类型断言当作验证。所有字符串（含 member name）累计 UTF-8 预算，容器深度最多64，全部 JSON 整数限定安全范围；拒绝重复键、非有限数、非法 Unicode、未知版本/字段。`ContractError.code` 不含正文或动态字段名。`fingerprint(command, limits)` 对已校验命令做 JCS/SHA-256，包含完整输入、expiresAtMs 和 commandId；可信 namespace 单独加入存储唯一键。键顺序不同不产生内容冲突。

## 三个行为 port

`ProviderAgentPort` 拥有 initialize/createSession/submit/cancel/respond/observe/reconcile/close；resume 和受控工具 verifier 是可选接缝。配置只传显式工作目录、config revision、账号引用、权限选择和宿主 ToolEndpoint，不将秘密写进 wire。ToolEndpoint 返回的是模型工具协议结果，不能签发批准或直连 runner。adapter 自己持有 SDK、进程、模型 transcript 和 native session/run/request ID。

能力分基础会话和受控工具 profile。基础可建立会话、文本多轮、输出/状态/终态和取消；取消能力区分 unsupported/unknown/request_only/terminal_acknowledged。跨进程 resume、steer、fork、子 agent、终端、结构化追问及多模态显式声明；扩展操作由具体 adapter 扩展接口承接，公共 V2 input 只接纳文本、取消和回答，不伪造通用多模态载荷。缺少能力不能请求对应操作。

能力声明必须绑定 provider/adapter version、config、账号及 generation；Host 在使用观察事件前核对完整 binding 与命令账本的 native run/request 关联。resume 必须核对 provider/config/account，跨 generation 仅在 across_processes 能力与 provider 成功响应后成立。展示历史不恢复模型上下文。`ControlledToolEvidence` 只能来自可信 adapter verifier；反序列化同形对象不能作为证据，仍需具体平台/版本封闭原生工具旁路。没有验证或可信身份时，Host 拒绝受控 profile。

`HostPort` 接受可信 ingress 提供的 Caller，提供 createSession、submit/cancel/respond、negotiate、snapshot/subscribe。Caller 是组合根的认证前置，不是本包签发的证书；禁止从模型内容、工具参数或 A2UI context 构造它。snapshot 包含稳定事件、命令、交互和 surface 与同一水位；超过明确输出上限返回 limit_exceeded，不静默裁掉历史。subscribe 从 exclusive cursor 回放后接实时；detach/AbortSignal 只结束订阅。过期 cursor、丢失事件或输出背压必须要求 resync，不伪造终态。

`SessionStore` 提供 create/session/accept/command/commit/snapshot/events/recovery/deliveries/retire/pruneRetired。accept 和 commit 是原子事务接缝；commit 核对 expectedRevision + expectedGeneration、推进 revision、提交稳定事件及必要 delivery，不接纳任意 async 事务回调。普通 commit 不得改变 provider/version、config、account、native session、generation 或 capabilities，运行坐标只能指向已确认 submitted 的活动命令 dispatch，清空须对应命令已 terminal；它不是跨 generation 恢复入口。interaction 的原命令及 native run/request、期限和 callbackLifetime 不可重绑。恢复查询和 delivery 查询有页大小与 opaque continuation，不承诺多 worker lease。

## 可靠性与交互

- namespace = tenant + principal + authority + logical session。相同 commandId/相同规范内容返回原 receipt；内容不同返回 content_conflict，不能覆盖旧记录。receipt 只有真实 store commit 后才能称 durable；测试替身的内存接纳不作此承诺。
- accepted 只表示持久接纳；dispatching 表示派发意图已保存；running 需要原生确认；terminal 需要明确模型终态。dispatching/running 结果不明进入 reconciliation_required，不能退回 accepted 自动重发。普通输入 queue_next；steer 必须精确匹配运行及能力。
- 取消已发送、模型 turn 已停止、进程已退出、业务副作用已停止分别记录。原生 transport/stream 错误不是 terminal；未知提交先核实原运行。`same_command` 重试仅复用同身份/内容；`reconcile_first` 禁止直接再次提交；`never` 不重试。
- 一个命令账本同时持有 inbox、内容绑定、派发意图和 native IDs，不复制第二份 provider 队列。事件日志支持重放，稳定事件先提交后发布；临时 token delta 没有 durable sequence，可丢弃/合并。
- Delivery 只用于需可靠跨服务交付的请求/结果，通过稳定 operationId/eventId、目标与内容摘要关联原事件。摘要固定为 `SHA-256(JCS({event, target}))`，由 `deliveryFingerprint(event, target, limits)` 生成，Store 必须核对完整引用事件和目标。接收方幂等；retry 类别为 receiver_idempotent/reconcile_first/never；未知副作用转核实，不自动重发。待交付记录和引用事件必须一起保留。
- retention.retryWindowMs > 0、receiptWindowMs >= retryWindowMs，精确截止时间写入 receipt。首次接纳拒绝过期 command；receipt 保留期内同内容重复返回既有接纳事实，不能据此重发模型命令。receipt 过期后仍保留键/摘要阻止复用，直到 namespace 退役并清理；达到容量上限应拒绝新接纳。仅所有命令 terminal、无 pending 交互方可退役，收据过期且 delivery 全结清后才能清理；旧 session ID 永不重建。
- interaction 固定 generation/native run/request、期限及 callbackLifetime。回答接纳与 pending→answered 在同一事务；第二个不同回答返回 already_answered，同命令重复返回 receipt。回调失效为 unavailable，重建 UI 不恢复 callback。provider_resumable 仍须 adapter 证明真实恢复；不能从序列化 capability 直接推断。

## ACP–A2UI 产品约定

ACP 固定官方 SDK 1.4.0 / schema-v1.21.0。标准 session/new、session/prompt、session/update、session/cancel、session/request_permission 保持上游语义；prompt 的最终响应必须有真实 stopReason，不能提前返回 accepted receipt。无 stopReason 的错误走标准错误路径，不制造成功响应。

产品能力在 capabilities._meta 的 `rss-mdm-agent.ai-runtime` 下协商 contractVersion=2、durableReceipts、cursorAttach，以及可选 A2UI version/catalogId/catalogVersion。`_rss-mdm-agent/submit`、`/snapshot`、`/attach` 和 `/update` 是扩展方法的完整产品前缀约定（代码中的 extension 常量为准），只有协商后使用；未知 request 按 ACP 返回 method-not-found，未知 notification 按上游规则忽略。A01 只冻结约定和 fixtures；实际 transport/协议 service 归 A04。

A2UI 固定 v0.9.1 snapshot，客户端 action、服务端 surface 生命周期、basic catalog 与 common types schema 原样保留在 [upstream](schema/upstream/a2ui/NOTICE.md)。`SurfaceBinding` 绑定 session/run、surfaceId、surfaceInstanceId、revision、interaction、component/event、catalog/version。create/update/delete 的上游 payload 不改写；产品扩展携带关联 metadata。一次新建 surface 使用新 instance ID，更新单调推进 revision，删除使旧 action 失效。snapshot 与事件恢复这些关联；renderer 和 catalog 内容校验由 A04 持有。

`resolveSurfaceAction` 校验关联与 Caller，返回的 context 仍是未经授权的回答数据；名称、timestamp、context 中的 actor/approved 均无批准权。metadata 中的 commandId 由产品客户端为该次回答生成并在重试时复用；它不是权限凭据。回答单次消费由 Host/Store 保证。无 A2UI 客户端保留文本、工具状态和标准权限交互；专有结构交互明确不支持，不能自动批准或用普通消息替代回答。

## 消费与验收

```sh
pnpm build:ai-contract
pnpm test:ai-contract
pnpm check:ai-contract
pnpm check:ai-consumer
cargo test -p ai-session-contract --locked
```

Node 验证基线24.14.1 / pnpm11.4.0。主导出是生成类型、编解码、ports 和协议关联函数；`/testing` 导出 FakeHost、MemorySessionStore、ScriptedProvider、共用 fixtures、runHostConformance、runStoreConformance 与 runProviderConformance。Host 工厂需提供零时钟、无终态的确定性 provider；Provider 工厂需提供 submitted/unknown 两种可控上游脚本，并结束观察流。套件会独立清理每个 provider，失败时保留主体和清理错误。共享场景用于后续真实 Host/adapter/stores 的同一验收入口；真实 SQLite 故障/崩溃、模型输出与工具旁路另外提供证据。

测试替身无 provider worker、进程调度、数据库或恢复后台任务。FakeHost 的固定窗口和1024条 snapshot 上限只是确定性测试配置。Rust 独立 consumer 和 TS tarball consumer 实际运行公共 API；不依赖 Tauri、相邻仓、原 workspace 的隐式依赖或 Rust 生成工具运行时。
