# ai-session-contract

通用 AI 会话、消息、工具提案和流事件契约，独立于 Tauri、模型 SDK、执行内核和 PR 业务。所有类型均为会话数据，不构造可信用户、批准或执行 Evidence。

## 消费方式

外部字节经 `decode_event(bytes, limits)` / `decode_command(bytes, limits)`；程序构造的对象使用 `validate(limits)`。`SessionLimits` 显式限制输入、文本总字节、内容分段和工具参数编码大小；无隐式无限配置。两个程序构造的 validate 入口也检查整体 envelope 的紧凑 JSON 编码长度；计数包含转义和 metadata，不分配第二份完整 payload，越界立即失败。

`EventEnvelope` 持有 V1、conversationId、sequence，事件引用稳定 turn/message/tool-call ID。ConversationStarted 内的会话 ID 必须匹配 envelope。sequence 使用 0 起的安全 JSON 整数；顺序、重复事件和状态归并由 host 持有，本库不实现调度器或持久会话状态机。

Message 是完整消息；MessageDelta 的 text 按相同 messageId 追加，尾部数据不代表完成。内容当前仅支持 Text，消费者以不可信文本渲染。Role 仅为展示/会话角色，不能认证产品用户；SendMessage command 仅允许 User，ToolCallResponse 使用独立 command。

`ToolCallProposal` 仅有 id、turnId、name、arguments。arguments 中的 actor/approved/readOnly 等键都是不可信工具数据；proposal 外添加 actor/approver/authorized 等字段直接拒绝。arguments 顶层、嵌套对象和数组中的对象递归拒绝重复键，直接 DTO 反序列化同样受此约束。host/MCP 依据目录 schema 映射到执行服务，不能直连 runner。工具响应 Returned/Rejected/Unavailable 仅表示回送模型的结果，不是 OS 执行证明。Debug 和错误不含原始正文或参数。

## 取消、恢复和能力

取消 command 与 CancelDispatched 回执独立于 TurnFinished。Accepted 仅确认请求派发，允许继续出现尾部消息；只有明确 TurnFinished::Interrupted 表示会话 turn 中断。StreamError（例如 TransportLost）不补造终态。任何会话状态都不证明进程树或外部副作用终止。

EngineCapabilities 的续接、中断、工具控制分别采用具体枚举。每一轴都有 Supported、Unsupported、Unknown；Unknown 是已定义的“能力未证实”，不是未来字段兜底。Supported 绑定 Conversation 内精确的 engine name/version/processGeneration 和 config id/revision。

- `satisfies(requirements)` 只进行能力匹配。AcrossProcesses 可满足 SameProcess；TerminalAcknowledged 可满足 RequestOnly；HostMediated 之外的工具模式不能满足宿主工具控制要求。
- `check_resume(conversation, binding)` 核对引擎/配置；SameProcess 必须匹配 generation，AcrossProcesses 可申请跨 generation 续接。返回 Ok 仅允许请求，实际 provider 仍可返回 MissingSession/EngineRejected。
- ResumeFinished 显式报告成功或不可恢复原因；恢复失败不清除执行任务，也不自动重新派发。
- 序列化的 Supported/HostMediated 本身没有证据权威。后续 adapter 必须对具体版本实际验证原生工具封闭与取消/恢复能力，host 才能开放受控操作。本 crate 无通用安全模式开关或自动批准设置。

EngineConfigRef 只存 id/revision，不含密钥、原始配置或权限覆盖。原始 provider 协议由对应 adapter 转换；本契约不保留任意 provider payload。未知 V1 字段、版本、事件、内容或终态变体均拒绝，不吞掉事件后继续声称成功。

## 验证与 schema

- `cargo test -p ai-session-contract --locked`
- `cargo run -p ai-session-contract --example ai-session-consumer --locked -- crates/ai-session-contract/tests/fixtures/events.json`
- `cargo run -p ai-session-contract --example ai-session-schema --locked`

固定 Draft 2020-12 schema 从 Rust 类型派生，golden 与真实解码/编码同时验证。schema 描述结构，文本预算、消息方向和关联一致性由纯 Rust 校验负责。events.json 包含彼此独立的协议案例和替代终态，不是可原样重放的单条会话日志；consumer 从中验证取消、尾部数据及中断的区别。

来源与参考重写边界见[来源记录](../../docs/reference/contracts-extraction.md)。协议 fixture 不证明真实引擎或原生工具隔离。
