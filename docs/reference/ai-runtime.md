# A01 AI Runtime 来源与版本

对应 #2439，范围基线 `ai-runtime-20260918`，查阅日期2026-09-18 UTC。产品新源码、schema 和 fixtures 为本仓重新实现，按 MIT 交付。

## 产品来源和替换

prmonitor 固定 `4dcc87264ad740da6559824e0a8b04a1c2914d4b`、rss-mdm 固定 `589211a598d588508375f7e84742cfa0dc7d29ce` 保持产品来源身份；A01 没有复制它们的源码或历史数据库。既有 C02 来源记录继续作为历史证据；A02/A03/adapters 后续若提取源码，应另列逐文件映射及许可，不把本记录当作整仓复制授权。

产品 wire 以独立 schema 为唯一声明，Rust/TS 消费生成物；历史手写模型由 Git 保留，不提供兼容入口。

## 固定一手上游

- ref: typify README.md / typify-impl/src/lib.rs@03e51a2785d6f4dc1bbc1b949f9eb99fda5e91a1 — [0.8.0 类型生成边界](https://github.com/oxidecomputer/typify/blob/03e51a2785d6f4dc1bbc1b949f9eb99fda5e91a1/README.md#L82-L109)。仅使用已验证的 JSON Schema 子集；生成器通过 syn 对 Debug 统一脱敏，未手改生成 DTO。
- ref: schemars schemars/src/schema.rs@v0.8.22 — [RootSchema 的 $defs alias](https://github.com/GREsau/schemars/blob/v0.8.22/schemars/src/schema.rs#L80-L101)。仅作为 typify 开发工具输入，不恢复 Rust→schema 真源。
- json-schema-to-typescript 16.0.0：npm artifact gitHead `7f72770eb854328c96b112be445da0306bebdbaf`；读取发布包 dist/src/index.js 的 compile 管线。TS 只生成类型，结构校验由 Ajv 8.20.0 承担。生成器的 TS 编译投影剥离引用节点的 description 以避免内联别名重复，再由 TypeScript AST 从同一 schema 补回字段文档；没有第二份模型声明。版本及完整性由 pnpm-lock 持有。
- ref: jsonc-parser src/impl/parser.ts@v3.3.1 — [visitor 接口](https://github.com/microsoft/node-jsonc-parser/blob/v3.3.1/src/main.ts#L229-L271)，在 JSON.parse 前拒重复键、非法输入与预算越界，不复制通用 parser。
- ref: canonicalize lib/canonicalize.js@v5.0.0 — [JCS 键序/Unicode 实现](https://github.com/erdtman/canonicalize/blob/v5.0.0/lib/canonicalize.js#L1-L75)。Rust 复用既有 serde_json_canonicalizer 0.3.2；共享 golden 核对摘要。
- ref: ACP TypeScript SDK scripts/generate.js@e6463f444093ed7c5f1cc937c3f32afb5853e906 — [SDK1.4.0 对应 schema-v1.21.0](https://github.com/agentclientprotocol/typescript-sdk/blob/e6463f444093ed7c5f1cc937c3f32afb5853e906/scripts/generate.js#L13-L14)，schema commit `272bf799f35a258c6a4107a0410ed361e83683d3`。从发布 SDK 读取原 schema 做协议 fixtures，未混用较新独立 schema release。
- ref: ACP docs/protocol/v1/extensibility.mdx@272bf799f35a258c6a4107a0410ed361e83683d3 — [扩展协商](https://github.com/agentclientprotocol/agent-client-protocol/blob/272bf799f35a258c6a4107a0410ed361e83683d3/docs/protocol/v1/extensibility.mdx)。产品字段使用命名空间，标准 prompt 最终响应不变。
- ref: A2UI specification/v0_9_1/json/client_to_server.json@04e6f07fde12ff2638b3b489bd9e3033066cb957 — [官方 action schema](https://github.com/a2ui-project/a2ui/blob/04e6f07fde12ff2638b3b489bd9e3033066cb957/specification/v0_9_1/json/client_to_server.json)。原样复制 client_to_server.json、server_to_client.json、common_types.json、catalogs/basic/catalog.json 与 LICENSE，目标为 ai-contract/schema/upstream/a2ui；逐文件 SHA-256 及固定来源见 [NOTICE](../../packages/ai-contract/schema/upstream/a2ui/NOTICE.md)。保留 Apache-2.0 许可；basic catalog 用于离线 surface 生命周期 fixture，不包含 renderer。

依赖与完整性由 lock 持有，来源记录不作为当前产品通过证明。

PR #1036 修复参考：ref: Node.js lib/internal/abort_controller.js（本地 Node24.14.1 内置源码的 AbortSignal.timeout/any）；signal 只发取消通知，conformance 另持有有界 watchdog 并保留清理错误。ref: serde_json src/error.rs@1.0.151（[一手源码](https://github.com/serde-rs/json/blob/v1.0.151/src/error.rs)），parser-level NumberOutOfRange 属于 Syntax，映射为公共 number 诊断并由共享正/负指数溢出 golden 固定。ref: TypeScript lib/tsc.js@5.6.2（本地固定包的 classPrivateFieldGet helper）；私有字段提供 nominal admission 边界，运行时构造另核验模块私有 token。新增 surface.status/response.surface 从唯一 schema 再生成 Rust/TS，没有手改生成物。

## A02 前置契约补充

参考 [Pi SQLite session backend](https://github.com/earendil-works/pi/blob/d981de1229ef899957bbe968bc8dcda02a21f477/packages/session-backends/sqlite-node/src/index.ts) 的同步数据库事务边界，将领域状态转换抽为生产同步函数供内存与 SQLite adapter 共用；未复制 Pi 源码或存储格式。新的 DispatchAttempt/observerGeneration、名义恢复证据与 surface 稳定事件是本产品契约，既有格式直接替换，不提供历史导入。

## ACP–A2UI 接入参考


查阅日期2026-09-19 UTC；本次新写产品适配代码，未提取 prmonitor 业务实现。既有 A01 固定协议 schema 及许可保持，详见[来源记录](../reference/ai-runtime.md)。

| 载体 | 固定值 | 一手来源/核对内容 |
| --- | --- | --- |
| ACP SDK / protocol | SDK1.4.0；schema-v1.21.0；protocol1 | ref: typescript-sdk `src/acp.ts` / `src/stream.ts`，发布包 `dist/acp.js` / `dist/stream.js`；[SDK commit](https://github.com/agentclientprotocol/typescript-sdk/tree/e6463f444093ed7c5f1cc937c3f32afb5853e906)，[protocol commit](https://github.com/agentclientprotocol/agent-client-protocol/tree/272bf799f35a258c6a4107a0410ed361e83683d3) |
| A2UI schema | v0.9.1，commit04e6f07fde12ff2638b3b489bd9e3033066cb957 | ref: a2ui `specification/v0_9_1/json`，原样 upstream schema/Apache-2.0 [NOTICE](../../packages/ai-contract/schema/upstream/a2ui/NOTICE.md) |
| Lit / web_core renderer | 两者 npm0.11.0；导入 v0_9 | ref: 发布包 `src/v0_9/processing/message-processor.js`、`state/surface-model.js`、`basic_catalog/components/Text.js` 和 Lit `surface/a2ui-surface.js`；npm 未提供 gitHead，不把它等同于 schema commit。实际运行验证与协议兼容，完整性由 pnpm-lock 持有 |
| Vue / Vite | 3.5.38 / 6.4.3 | ref: [Vue Web Components](https://vuejs.org/guide/extras/web-components)，浏览器接入验证 property 接缝与 custom element |
| 静态校验/摘要 | Ajv8.20.0 / @noble/hashes2.0.1 | ref: [Ajv standalone](https://ajv.js.org/standalone.html)，[noble sha2.ts](https://github.com/paulmillr/noble-hashes/blob/2.0.1/src/sha2.ts)，共享 golden 保持摘要一致 |

renderer npm 版本、协议版本、导入路径、产品 catalog revision 是四个独立标识；不得互相推断。未知组件或官方处理器异常先失效再恢复，官方逐条内存更新不是事务提交。
