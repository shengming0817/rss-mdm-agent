# ACP–A2UI 开发与验证

本仓 A04 接缝由 [ai-access](../../packages/ai-access/README.md)、[ai-client](../../packages/ai-client/README.md)、[ai-ui-bridge](../../packages/ai-ui-bridge/README.md) 提供。唯一产品 wire 仍由 ai-contract schema 生成 Rust/TS；页面归 #2409，真实 Host/SQLite/providers 和最终 Tauri 装配分别由既定 owner 持有。

## 消费顺序

1. 可信组合根创建 `createAccessService({ host, sessionOptions })`，绑定认证后的 caller 与官方 SDK Stream。不要从请求正文提取 tenant/principal/authority。
2. `RuntimeClient(stream)` 协商扩展，创建/列出/恢复会话。标准 ACP consumer 可以完全不使用产品扩展，prompt 仍等待终态。
3. Vue 使用 `RuntimeSurface` 挂载指定 surface instance，或使用 `createSurfaceRenderer`。renderer 的 processor 只持有显示缓存；客户端投影持有稳定恢复内容。
4. 处理 `resync_required` 时调用 `restore`。分页 token 是有期限且绑定 caller/查询的不可猜测凭据，仅为读视图定位，不授予身份。内存 double 与 SQLite 共用读视图，默认30秒/最多128份及16MiB保留内容，未签发 continuation 的完成单页立即释放配额，已签发的多页视图保留到 TTL 支持 token 重试；单页最多256条且同时受总字节/文本/深度/节点预算约束。

标准权限由 `requestPermission` 将实时回调递送到附着客户端；普通问题仍通过 Interaction/Surface 回答。恢复历史不会重新建立 native callback。断开/卸载与取消分别操作，不自动取消原生执行。

## 直接替换范围

本次已集成 #1047/#1050 的恢复契约和真实 SQLite adapter；A01 #2439 已完成，本次 #2442 在同一 PR 内演进其 owner 契约。删除单次 `snapshot`/`Snapshot` 和 `maxSnapshotRecords`，以 `snapshotPage`、`PageQuery`、`SnapshotPage` 替代；加入 caller 范围的 session 列表、surface 读取及 resume port。`SurfaceBinding` 直接替换为 `SurfaceState`，必须持有完整有界官方恢复消息，其稳定事件与投影同批提交。`Outcome` 删除 `interrupted`/`limit_reached`，改为 `cancelled`/`max_tokens`/`max_turn_requests`，唯一终态直接映射 ACP stopReason。Capabilities 新增明确 queue 能力。

当前产品 wire 为 V5；C16 删除旧 accepted 事件形状，C20 加入持久交付请求与回执事件；旧版本直接拒绝；没有 alias、旧 reader、双写、历史数据迁移或另一个浏览器 codec。新建产品扩展、协商 DTO 与冻结 catalog identity 全部由同一个 schema 生成，selection 在双端只能减少 offer 能力。pending 事件/快照共同保留权威期限和 callback lifetime，answered 保留首答 commandId；没有旧事件兼容 reader。通用入口使用静态 Ajv 校验器及固定 noble SHA-256；文件读取、Ajv 编译只发生于生成脚本。Node 恢复准入通过明确的 `/session` 入口消费，持久化同步规则通过 `/transitions` 消费，两者不进入浏览器依赖闭包。生成的校验器中的纯 helper 来自固定 Ajv 包，不依赖浏览器运行时 CommonJS。

## 验证入口与证据范围

```sh
pnpm install --frozen-lockfile
pnpm test:ai-contract
pnpm test:ai-access
pnpm check:ai-boundaries
pnpm check:ai-access-consumer
```

最后一项打包四个包，在系统临时目录建立独立 workspace，离线安装固定依赖；以官方 SDK/ndJsonStream 验证标准 ACP，再实际构建并运行 Vue + Lit consumer。Vite 解析钩子拒绝 Node 内置依赖；隔离 TypeScript consumer 检查公开 renderer factory 与事件类型；Chromium 验证四种 Surface 操作、卸载重挂、加载/渲染失败重试、转义文本及回执丢失后的同命令重试，以及过期自动禁用、权威期限上限、竞争 loser 清理、callback 失效后的迟到错误隔离。macOS 默认使用系统 Chrome；其它平台通过 `AI_BROWSER_PATH` 指定 Chromium 可执行文件。缺浏览器会失败，不降级为 SSR。Playwright-core1.58.2 仅用于本地测试驱动，不下载浏览器、不进入产品依赖。

测试 Host/Store 明确是内存 double，协商固定返回 durableReceipts=false；脚本提供 accepted、dispatch、terminal、question 与 surface 证据。它们证明接入和消费语义，不证明真实 provider、持久化回执、工具隔离或 OS 执行。独立 consumer 的 loopback HTTP 只是测试通道，不代表远程 HTTP/relay 互操作成熟。收尾在已提交源码上执行本仓 `make ci CI_BASE=origin/develop`，证据由 `.local-ci-runs/latest.json` 绑定实际 SHA/lock/运行环境。

## 上游版本与来源

查阅日期2026-09-19 UTC；本次新写产品适配代码，未提取 prmonitor 业务实现。既有 A01 固定协议 schema 及许可保持，详见[来源记录](../reference/ai-runtime.md)。

| 载体 | 固定值 | 一手来源/核对内容 |
| --- | --- | --- |
| ACP SDK / protocol | SDK1.4.0；schema-v1.21.0；protocol1 | ref: typescript-sdk `src/acp.ts` / `src/stream.ts`，发布包 `dist/acp.js` / `dist/stream.js`；[SDK commit](https://github.com/agentclientprotocol/typescript-sdk/tree/e6463f444093ed7c5f1cc937c3f32afb5853e906)，[protocol commit](https://github.com/agentclientprotocol/agent-client-protocol/tree/272bf799f35a258c6a4107a0410ed361e83683d3) |
| A2UI schema | v0.9.1，commit04e6f07fde12ff2638b3b489bd9e3033066cb957 | ref: a2ui `specification/v0_9_1/json`，原样 upstream schema/Apache-2.0 [NOTICE](../../packages/ai-contract/schema/upstream/a2ui/NOTICE.md) |
| Lit / web_core renderer | 两者 npm0.11.0；导入 v0_9 | ref: 发布包 `src/v0_9/processing/message-processor.js`、`state/surface-model.js`、`basic_catalog/components/Text.js` 和 Lit `surface/a2ui-surface.js`；npm 未提供 gitHead，不把它等同于 schema commit。实际运行验证与协议兼容，完整性由 pnpm-lock 持有 |
| Vue / Vite | 3.5.38 / 6.4.3 | ref: [Vue Web Components](https://vuejs.org/guide/extras/web-components)，实际 consumer 验证 property 接缝与 custom element |
| 静态校验/摘要 | Ajv8.20.0 / @noble/hashes2.0.1 | ref: [Ajv standalone](https://ajv.js.org/standalone.html)，[noble sha2.ts](https://github.com/paulmillr/noble-hashes/blob/2.0.1/src/sha2.ts)，共享 golden 保持摘要一致 |

renderer npm 版本、协议版本、导入路径、产品 catalog revision 是四个独立标识；不得互相推断。未知组件或官方处理器异常先失效再恢复，官方逐条内存更新不是事务提交。

## 失败与资源生命周期

Host 单次调用、权限整次分发与各 peer 请求复用 A01 `withinBudget`：作用域完成时移除父 signal 监听、清理 watchdog 并取消剩余等待者。caller、service、session cancel 与 pump detach 各自保持取消权；无效 option 不赢得首答。订阅使用 pump 生命周期，prompt 的终态等待不受单次 Host 接纳预算截断。没有新增 wire、预算接口或授权权威。

`tests/ai-access/permission-ownership.test.mjs` 固定 Node 24.14.1，使用真实 ACP SDK 双端 consumer 与 FakeHost：20,000 次允许/拒绝交替交付，每 1,000 次换轮后 GC 并检查 service/caller/pump 监听与 Node dependent graph 回到基线、无残留 watchdog；另覆盖 200 次超时、200 次双端 detach/restore、无效首答与 Host 请求作用域释放。此证据是协议与生命周期压力回归，不代表真实模型或 OS 执行验收。机制参考：[Node v24.14.1 abort_controller.js](https://github.com/nodejs/node/blob/v24.14.1/lib/internal/abort_controller.js) 的 composite source/dependant 链接。

服务的 `shutdownTimeoutMs` 独立于接纳 `timeoutMs`，默认5秒。取消后有界等待自有任务；`cleanup_timeout` 表示未全部结算，不能据此宣称 provider 终止，组合根承担残留隔离。resume 停止旧订阅后从返回 Session 的 lastSequence 重建，产品 attachment 先进入 resync_required；客户端显式 resume 则先 detach，再恢复新视图。

客户端公共失败使用 ClientError，renderer 使用 RendererError；均保留闭合 code，不转发底层 payload/cause。observer 失败与诊断 callback 失败彼此隔离。分页先收齐同水位历史再覆盖权威投影。交互期限是含端点的 UTC 毫秒，UI 的本地过期展示不改写 pending 的权威状态；answered 仅为本地同 commandId 保留回执重试，其他终态展示原因并禁用。

本轮生命周期机制参考：[ACP SDK 固定 jsonrpc.ts](https://github.com/agentclientprotocol/typescript-sdk/blob/e6463f444093ed7c5f1cc937c3f32afb5853e906/src/jsonrpc.ts) 的协作取消与闭合 RequestError；[Node v24.3.0 abort_controller.js](https://github.com/nodejs/node/blob/v24.3.0/lib/internal/abort_controller.js) 的取消/期限；[固定 A2UI processor](https://github.com/a2ui-project/a2ui/blob/04e6f07fde12ff2638b3b489bd9e3033066cb957/renderers/web_core/src/v0_9/processing/message-processor.ts) 的版本与 action 接缝。产品 owner 补充有界结算、权威交互期限和回执恢复，不把 upstream 的内存显示状态当作持久化事实。
