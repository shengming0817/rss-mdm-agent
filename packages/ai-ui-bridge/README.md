# Vue / A2UI 接缝

Vue 消费 `RuntimeSurface`：传入 `runtime`、`sessionId`、`instanceId`，它读取唯一客户端投影，挂载官方 `a2ui-surface`，提交有界 action，并发出 `receipt`/`error`。`useSessionView` 提供 Vue shallowRef；`createSurfaceRenderer` 提供独立 DOM 接缝。页面不需要另写消息处理器或恢复状态机。

固定 `@a2ui/lit`、`@a2ui/web_core` 均为0.11.0；导入 `/v0_9`，processor 明确使用协议 v0.9.1。产品 catalog `urn:rss-mdm-agent:a2ui:interaction` revision1 只含 Text、Column、Button、TextField，保留官方消息结构。无函数、动态 children 模板、资源组件、theme 或 sendDataModel；不提供 Markdown renderer，文本按 Lit 转义显示。Vue 编译器可将 `a2ui-*` 配置为 custom elements；组件内部用 DOM property 提供官方 SurfaceModel。

每次输入先校验完整恢复消息，再创建可丢弃的官方 processor。处理异常销毁局部缓存；卸载也销毁，重挂从 ai-client 恢复内容。未知 catalog/组件和 renderer 加载失败显示明确错误，标准文本与权限通道仍可使用。用户输入保留在当前 renderer 缓存直到提交；后续稳定 Surface 更新会从 Host 内容重新建立缓存。

`replace(surface, interaction, connected)` 同时消费稳定 SurfaceState 和 InteractionView。action 携带固定 commandId、expiry、surface revision、interaction/run/generation，请求期限取权威交互期限与本地30秒预算的较小值。`RuntimeSurface` 的 `now` 和 renderer option `now` 默认使用 Date.now，可注入一致的测试时钟；到期定时禁用并说明原因，点击时再检查。失败 `retry()` 复用同一请求。挂载组件提供 Retry response / Retry card 按钮，并暴露 `retry` 与 `remount`。失去回执后，仅当 answered 的 responseCommandId 等于本地待决命令时保留重试以取回原回执；其他客户端胜出、expired、unavailable 或 terminal RPC 拒绝会取消无效重试；迟到失败不能复活已失效按钮。不会由重复点击重新生成 commandId。公开 `SurfaceRendererHandle` / `RendererFactory` 可用于替代 renderer，`canRetry` 暴露当前请求可否恢复，error/receipt 事件分别带闭合的 RendererError/Receipt 类型。RendererError.code 区分加载、内容、已销毁、暂时 action 失败与确定拒绝；failure 只保留 ClientErrorCode，不携带原始异常或 payload。模型内容及回答只是不可信数据，不构成可信计划或执行批准。独立 Vue consumer 的四种操作、恢复与真实输入回传通过 `pnpm check:ai-access-consumer` 验证。

`questions(request)` 将有界普通结构化问题投影为 question/header/options/multiSelect，未知形状返回 undefined；它只做基本展示归一化，回答仍由 provider adapter 验证，不签发权限或批准。
