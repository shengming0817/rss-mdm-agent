# AI 客户端

`RuntimeClient(stream)` 支持浏览器标准 Web Streams，无 Node 运行依赖。先 `initialize()`，再 `createSession()` 或 `restore(sessionId, pageLimit)`；`listSessions({ limit, continuation })` 返回 caller 范围内的分页。

恢复时先读完整、同 snapshotId/revision/cursor 的分页到临时投影，通过后接续 cursor 并原子发布。`observe(listener)` 和 `getSession(id)` 提供克隆的公共 `SessionView`。同连接多会话各自维护 attachmentId，旧连接/旧挂载消息不会写入当前视图。重复稳定事件忽略；序列缺口、过期游标或非法内容使视图进入 `resync_required`，调用者用 `restore` 明确恢复。临时 delta 不能覆盖稳定文本或终态。receipt 和标准 ACP 通知不写这份投影。

`submit(command)`、`action(request)` 保留调用方的稳定 commandId、内容及 expiry；重试复用整个请求。`resume` 与恢复显示独立。`detach`/`close` 不取消模型运行。列表、恢复、错误状态及 UI 接缝不需要页面另写 replay/bridge。

`channelStream({send, listen})` 将组合根提供的双向消息通道变成 SDK Stream；listen 必须递送断线通知并返回清理函数。SDK 自己负责请求关联与 callback，adapter 仅做有界传输。`localTransportPair` 及官方 `ndJsonStream` 用于无 Tauri 消费；真实桌面接线归 #2413。

`requestPermission` 可选回调收到官方请求与 AbortSignal；默认取消，不能当作执行审批。fake Host/脚本事件由 `@rss-mdm-agent/ai-contract/testing` 提供，其内存回执不证明 crash durability。
