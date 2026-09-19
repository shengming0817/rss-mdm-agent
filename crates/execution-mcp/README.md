# 受控执行 MCP 适配器

C17 / #2410。该 crate 将 MCP 工具映射到宿主绑定的执行服务 port，不持有批准、执行、幂等账本或任务状态机。
生产依赖为 execution-contract、service-catalog、rmcp 和必要的协议/异步基础库。没有 UI、模型 SDK、SQLite、shell 或 runner 依赖。

## 唯一入口与版本

`ExecutionMcp::new(Arc<impl ExecutionServicePort>, McpLimits)` 在接收输入前验证配置和服务绑定；
`serve(reader, writer, stop)` 接收宿主已经持有的 stdio/字节流。库不创建子进程或网络监听器。
公开对象不实现 rmcp ServerHandler，因此消费方不能绕过有界 transport 直接挂载内部 handler。

- rmcp 固定为 3.4.0，禁用默认 features，生产启用 server；client 仅用于开发验证。
- 协议声明与支持集合均只有 MCP 2025-11-25。遵循该版本的标准初始化协商，不保留其它版本的业务投影、别名或降级路径。
- 只声明 tools 能力。七个工具固定，无动态目录工具注册，无批准工具。
- 输入/输出 schema 从 Rust 类型派生，使用 JSON Schema Draft 2020-12。输出使用 structuredContent；不为旧客户端重复生成文本 JSON 副本。

| 工具 | 参数与结果 |
| --- | --- |
| execution_catalog | 空对象；返回授权目录、精确 catalog 引用和 C03 参数 schema |
| execution_capabilities | 空对象；返回 supported/blocked/unsupported/unknown 和静态原因 |
| execution_preview | catalog 选择或 candidate 精确引用；返回既有 C01 计划 ID/摘要及能力投影 |
| execution_propose | catalog 选择或有界 script 草稿；返回不可变 candidate 引用 |
| execution_submit | operationRequestId + 精确 plan；返回接纳/执行事实投影 |
| execution_status | operationRequestId；返回授权命名空间内的原任务 |
| execution_cancel | operationRequestId；返回业务取消接纳和当前状态 |

候选/预览采用外部标签枚举，例如：

```json
{"catalog":{"selection":{"operationRequestId":"request-1","catalog":{"authority":{"kind":"test","id":"catalog-test"},"identity":{"id":"self-service","revision":"r1"},"digest":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},"itemId":"diagnostics","variantId":"network-check","arguments":{"host":"example.invalid","count":3}}}}
```

示例 digest 仅展示结构；实际必须使用服务返回的精确引用。
script 输入为 `{"script":{"operationRequestId":"...","sourceUtf8":"...","interpreter":<ExactArtifactRef>}}`；
candidate 预览为 `{"candidate":{"operationRequestId":"...","candidate":<ExactArtifactRef>}}`。
脚本草稿是待授权的 UTF-8 内容，服务负责保存、冻结、解释器核验与后续规划，MCP 不执行它。

外部标签使 serde 直接解码选定变体。内部标签/untagged 的中间值缓冲会破坏 RawValue；
原始 arguments 必须经私有非 wire Extensions 到达 handler，再由 C03 `select` / `ParameterProjection` 校验。
因此重复键、数字舍入、未知参数、默认值和秘密引用不会因 MCP 入口而获得另一套语义。

## 执行服务合同与信任边界

宿主创建的 port 实例唯一持有可信 authority/tenant、actor、device、delegation 和来源。
模型参数、MCP request ID、`_meta`、clientInfo、provider 登录及工具注解都不能构造这些事实。
实际身份认证在宿主/执行服务完成；trait 的存在不能证明任意实现已做认证。
没有真实绑定的生产 port 必须返回 Unbound。调用过程中仍须复核授权和时效，不能把启动检查当作永久许可。

每个 port 方法，包括目录与能力读取，都必须执行与传统 UI 相同的业务校验。
目录由服务限制可见范围，核心 FrozenCatalog 只证明数据校验，不证明发布方或调用主体可信。
跨命名空间查询不能泄露对象是否存在。输出只携带授权投影/证据引用；服务不得把秘密放入目录展示字段或标识。
Expired、Denied、NotFound、Conflict、Unsupported、Unavailable 等分类由服务返回；
目录错误投影为闭合 `catalogReason` 对象：`kind` 区分类别，参数/定义错误带 `rule`，预算错误带 `coordinate`。
例如 `{"kind":"invalidArguments","rule":"roundedNumber"}`、`{"kind":"limitExceeded","coordinate":"parameters"}`；
这些字段均由静态枚举映射并进入 outputSchema，不回显输入或底层错误，也不要求客户端解析 Rust Debug。

业务幂等只由执行服务拥有：

- operationRequestId 与可信命名空间、完整规范请求内容绑定；同内容重放返回原任务/attempt，内容改变返回 Conflict。
- 预览冻结后的原始主体与来源绑定保持不变；不同 RPC ID、重试或连接不能改写原计划或原始来源。
- Accepted 只表示已接纳。ExecutionEnded、Verified、OutcomeUnknown、TestCompleted 分别表达不同事实。
- 回包丢失、超时和断线后，用原 operationRequestId 查询或重试，不分配新尝试。
- port 必须明确其持久幂等保留期；记录过期不能把一个可能已执行的旧 ID 静默当作新执行。
- CancellationToken 只结束 RPC 等待。已接纳任务独立于 MCP future/进程存活；真实业务取消必须调用 cancel 方法。
- Requested 不证明进程终止或效果回滚；终态取消返回 AlreadyTerminal。

工具返回 `{status:"ok",result:...}` 或 `{status:"error",error:{code,catalogReason}}`，
与各工具 outputSchema 一致。unknown tool/协议形状错误使用 JSON-RPC 错误；无法安全解码的输入关闭连接。
内部 `ToolKind` 单源持有工具名称及副作用分类，注册/分派/超时均消费该类型。
目录、能力和状态声明只读，超时返回 unavailable，可重新读取。
候选、冻结预览、提交和取消声明非只读：它们可能已持久化，超时返回 outcomeUnknown；调用方使用原 ID 查询或重试，不能据此声称已执行或未执行。

## 预算、生命周期与日志

McpLimits 为必填宿主配置，没有生产默认值。包含帧/响应字节、JSON 深度/节点、
并发请求、会话总帧、请求时限、输入空闲/不完整帧及输出写出时限，以及已有目录/参数预算。
示例的数字只适用于测试。单个字符串也受帧字节总预算限制，目录参数另受 C03 字符串预算限制。

有界 LinesCodec 先组帧，再检查完整 JSON 重复键/结构，之后才由 rmcp 解析消息。
许可在 SDK 调度前获取，覆盖业务等待和最终响应写出；忙请求在 transport 直接返回错误。
响应在受限 writer 中编码，超限用静态协议错误替代；partial write/超时后关闭连接，不接着写入另一帧。
取消后的协议 ID 在本连接内隔离，避免 SDK 的晚响应污染同 ID 新请求；正常完成后的 ID 可复用。
业务 operationRequestId 不受该连接级规则影响。
会话总帧预算同时约束通知、拒绝请求及取消 ID 隔离记录；耗尽后重连仍使用原业务 ID。
显式 stop、EOF 和协议失败触发端口等待取消与许可排空；丢弃 serve future 也会取消会话。
正常 EOF/显式 stop 返回 Ok；协议错误、帧超限和读写失败保留首个静态原因，清理后由 serve 返回 InvalidInput、Limit 或 Unavailable。

交给 rmcp 前清除工具 arguments、请求/通知 metadata、取消 reason 和未消费的请求续传字段；
业务原文只通过不暴露 Debug 内容的私有 Extensions 传递。真实 TRACE subscriber 回归测试验证脚本、秘密引用和 metadata/reason 不进入 SDK 日志。
rmcp 仍会记录协议关联标识、clientInfo 和授权结果投影。**宿主日志 subscriber 必须过滤这些 rmcp 内容日志**，
不能直接接受任意 RUST_LOG 环境过滤表达式；服务不得在标识或授权展示投影中放入秘密。
库不覆盖全局日志策略。真实进程 consumer 的 stderr 检查仅证明该测试 launcher 的输出纪律，不替代带 subscriber 的回归测试。
stdout 只写 MCP；所有错误和本 crate 的诊断保持静态、脱敏。

## 原生 provider 的交接

provider 范围与任务映射由[产品 PRD 的引擎基线](../../docs/product/rss-mdm-agent-prd.md#provider-baseline)持有。
具体 provider adapter/组合根直接映射其固定版本的原生配置，并验证可信 launcher 来源、身份绑定、
协议版本及原生工具限制；#2412/#2413 负责真实执行服务和桌面装配。
本 crate 只接收宿主持有的字节流，不公开尚无生产消费者的 launcher 配置描述。
测试 consumer 直接启动自身 TEST-only 子进程，不代表 provider 接线或工具旁路封闭验证。
若 provider 不支持本协议基线，应报告不支持。

## 验证与来源

```sh
cargo test -p execution-mcp --locked
cargo run -p execution-mcp --example mcp-consumer --locked -- crates/service-catalog/tests/fixtures/catalog.json crates/execution-contract/tests/fixtures/plan.json
node scripts/check-rust-consumers.mjs
make ci CI_BASE=origin/develop
```

protocol 测试通过真实 rmcp server 验证参数、身份边界、幂等/丢响应、取消、并发、超限及慢读；
另有官方 rmcp client 互通。mcp-consumer 自启 TEST-only 子进程，通过 stdin/stdout 调用全部七个工具；
同一源码由公共 consumer 清单在独立 workspace/lock/target 中运行。子进程启动仅在验证 consumer 内。

TestService 为显式测试 authority、内存存储和预设事实，没有数据库或 runner。
其跨 adapter 重连测试不证明跨进程持久性、SQLite 原子性、真实身份、三引擎旁路封闭或 OS 执行权限。
真实执行服务接线须用本接口合同重跑其自身持久化/故障证明。

本 crate 为原创协议适配，未复制 prmonitor 源码、数据或业务；上游通过 Cargo 包消费，MIT/Apache-2.0 许可随原包保留。
固定对标（已读取源码）：

- ref: rmcp transport/async_rw.rs@fd7811fdaa9fefa1c8034534b4d7a31c97204f89：[默认整行累积](https://github.com/modelcontextprotocol/rust-sdk/blob/fd7811fdaa9fefa1c8034534b4d7a31c97204f89/crates/rmcp/src/transport/async_rw.rs#L125-L160)，本 adapter 补齐 decode 前预算。
- ref: rmcp service.rs@fd7811fdaa9fefa1c8034534b4d7a31c97204f89：[请求分派与取消](https://github.com/modelcontextprotocol/rust-sdk/blob/fd7811fdaa9fefa1c8034534b4d7a31c97204f89/crates/rmcp/src/service.rs#L1564-L1659)，本 adapter 补齐入站许可与等待期限。
- ref: rmcp handler/server.rs@fd7811fdaa9fefa1c8034534b4d7a31c97204f89：[版本支持接口](https://github.com/modelcontextprotocol/rust-sdk/blob/fd7811fdaa9fefa1c8034534b4d7a31c97204f89/crates/rmcp/src/handler/server.rs#L378-L400)，显式限制实际支持集合。

- ref: rmcp-macros src/tool.rs@fd7811fdaa9fefa1c8034534b4d7a31c97204f89：[工具元数据](https://github.com/modelcontextprotocol/rust-sdk/blob/fd7811fdaa9fefa1c8034534b4d7a31c97204f89/crates/rmcp-macros/src/tool.rs)，本 adapter 用私有 typed descriptor 绑定元数据与分派，保留原始参数预算路径。
- ref: rmcp src/model.rs@fd7811fdaa9fefa1c8034534b4d7a31c97204f89：[结构化错误与结果](https://github.com/modelcontextprotocol/rust-sdk/blob/fd7811fdaa9fefa1c8034534b4d7a31c97204f89/crates/rmcp/src/model.rs)，目录诊断使用 adapter 自有的闭合 wire 投影。
