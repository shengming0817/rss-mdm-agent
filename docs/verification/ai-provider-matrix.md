# 三引擎公共能力与恢复验收

[A06 #2444](https://dev.azure.com/shengming0923/rss/_workitems/edit/2444) 验收现有 A01、Host、SQLite、ACP/A2UI 和三个原生 adapter 的组合。公共契约保持 V4；无旧版解析、兼容分支、第二份能力注册表或新的执行权威。

## 运行与证据

`pnpm test:ai-acceptance` 构建现有 Host 依赖，自动发现并串行运行 [共同套件](../../tests/ai-provider-conformance/) 和 [进程恢复套件](../../tests/ai-recovery-integration/)。必须有三个引擎及所有指定场景的实际断言与诊断；少测、跳过、失败、重复证据、源码未提交或运行中改变均不能得到通过结论。每行必须匹配固定版本、配置、能力、实际工具清单和 adapter profile/plugin 源码摘要，缺失或错误绑定同样失败。本入口使用真实原生进程和本地模型协议服务，不需要外部账号。

回执位于被忽略的 `.local-ci-runs/ai-provider-matrix.json`，包含命令、UTC、前后源码 SHA/基线/clean 状态、两份 lock 摘要、Node/OS/架构、各测试结果、每个已准入会话的 provider/adapter 版本、配置 revision、generation、原生关联 ID、实际工具列表和恢复结果。已准入场景必须有非零模型请求，工具清单从该场景所有真实请求提取、排序去重；Codex 命名空间展开为 `namespace__tool`，Claude/DeepSeek 从各自原生字段读取。conversation 清单分别为 Codex 空集、Claude `AskUserQuestion`、DeepSeek `ask_user_question`；Codex controlled_tools 包含 `mcp__rss_host__propose` 和固定的三个空资源辅助工具。清单缺失、多余、缺项、重复或顺序不规范均不能通过；拒绝准入行要求零请求且不伪造工具观察。临时路径与凭据不进入回执；工具/plugin 组合另由该源码的固定 profile 与 lock 绑定。回执必须与当前 clean SHA 和 lock 一致，历史回执不能证明当前构建。

完整验收运行 `make ci CI_BASE=origin/develop`，同时执行下表指向的原有权威测试；共同套件没有复制 Rust 批准或 SQLite 事务算法。最终回执和全量本地结果随 PR 留痕，本文记录方法和范围，不充当滚动通过看板。运行平台目前是产品 Host 支持的 macOS arm64；其他平台失败不能记作跳过后通过。

## 能力与准入分开判定

表中是固定实现应满足的契约，实际运行结果读取回执。supported 表示对应场景已执行且断言成立；unsupported 必须是明确拒绝；unknown 表示没有可支持肯定结论的证据。取消确认与业务结果分别记录，unknown 不授权重发。

| 能力 / 产品 profile | Codex | Claude | DeepSeek |
| --- | --- | --- | --- |
| 原生版本来源 | `@openai/codex@0.155.0` | Agent SDK `0.3.277` / 原生 CLI 启动实测 | Harness `0.1.6-alpha.2` / 固定组合摘要 |
| conversation | supported；原生业务工具为空 | supported；只保留结构化问题 | supported；只保留结构化问题 |
| Host FIFO、多入口同 commandId | supported | supported | supported |
| Host steer 入口 | supported | unsupported | unsupported |
| 原生 adapter fork 扩展 | supported；独立原生测试 | unsupported；无该扩展 | unsupported；无该扩展 |
| 结构化问题 | unsupported | supported | supported |
| 取消契约 | request_only | request_only | request_only |
| 原生跨进程上下文续接 | across_processes | across_processes | across_processes |
| 产品 controlled_tools 准入 | 仅当前固定 Codex + darwin-arm64 | unsupported | unsupported |
| 组件 host_mediated 测试 | 独立适配器测试 | 独立适配器测试，测试 verifier | 独立适配器测试，测试 verifier |
| 任意同 UID 文件/网络/IPC 隔离、Windows、真实 OS T3 | unknown | unknown | unknown |

组件工具协议成功不改变产品准入。共同套件在真实 Rust MCP 接入下验证产品装配：Codex 只能通过 Host proposal 读取 Rust 目录；其余两家创建受控会话被拒绝，模型请求数为零。该 Rust 服务使用明确 S1 组合，不证明安装成功或真实 OS 执行。

## 场景与权威入口

| 场景 | 入口与断言 |
| --- | --- |
| 六个连续轮次、流式稳定消息、FIFO、双客户端同 ID 重试与内容冲突、断线分页重放 | [native-host.test.mjs](../../tests/ai-provider-conformance/native-host.test.mjs)：真实 Host/SQLite/worker/native；不同主体不能读/submit/cancel/respond；不匹配的 provider/account/config/profile 不能新建 |
| steer 与 queue_next 差异 | [steering.test.mjs](../../tests/ai-provider-conformance/steering.test.mjs)：另一客户端对正在运行的 Codex turn 输入 steer，独立请求收到 ACK、原 turn 只结束一次；Claude/DeepSeek 在 Host 入口明确拒绝且不派发 |
| 取消请求、单独 ACK、实际原生终态 | [cancellation.test.mjs](../../tests/ai-provider-conformance/cancellation.test.mjs)：各引擎实际取消；DeepSeek 活动问题收到原生取消终态，模型传输未返回即关闭 Host 的情况则恢复为 unknown，原请求重试不重发 |
| 两个入口抢答、相同回答重试、迟到回答、旧 generation | [questions.test.mjs](../../tests/ai-provider-conformance/questions.test.mjs)：真实 Claude/DeepSeek 回调只消费一次；重启保留展示内容但旧回调 unavailable |
| 产品受控工具与固定准入 | [controlled.test.mjs](../../tests/ai-provider-conformance/controlled.test.mjs)：生产装配、实际 Rust MCP、模型强制 proposal；不同引擎按真实准入结论判断 |
| provider 已收到而 Host 尚未提交事实 | [native-recovery.test.mjs](../../tests/ai-recovery-integration/native-recovery.test.mjs)：在 Store.commit 前精确暂停，确认真实模型请求后 SIGKILL Host；清理旧进程组、重开 SQLite，只核实原 attempt，未知不成为成功、不新增请求 |
| Host 重启、展示与原生上下文独立 | 同上：restore 仅重放，显式 resume 才产生新 generation；下一次显式 prompt 的原生请求包含此前 nonce，原生 session/thread 保持一致 |
| 派发前事务故障、重复事件、恢复 CAS、实例所有权 | [AI SQLite tests](../../tests/ai-store-recovery/) 与 [Host recovery](../../tests/ai-host/recovery.test.mjs)：真实 SQLite 和进程边界；脚本化 worker 只用于公共状态机故障注入 |
| Rust 已提交但 AI Host 没收到回执 | [execution-faults.test.mjs](../../tests/ai-host/execution-faults.test.mjs)：真实两个 SQLite、Rust 子进程；恢复仍为同一个 Rust attempt、投递不重发；进程退出与取消状态独立。不存在跨库原子事务承诺 |
| A2UI 历史卡片可重建且不能恢复权限 | [SQLite access](../../tests/ai-store-recovery/access.test.mjs) 实际重开、保留原始卡片消息并撤销动作；[access actions](../../tests/ai-access/actions.test.mjs) 验证过期/删除/旧 generation/跨主体与并发动作。卡片内容和回调在不同层验收，不声称三家原生引擎产生 A2UI |
| 批准不重复消费、任务不重复尝试、跨设备/主体/配置不能复用 | [Rust acceptance](../../crates/execution-app/tests/acceptance.rs)、[desktop composition](../../apps/desktop/src-tauri/tests/composition.rs) 与执行批准核心；设备绑定属于 Rust 执行权威，不向 AI wire 添加设备授权字段 |
| unsupported / unknown 不显示或调用高级能力 | [access capabilities](../../tests/ai-access/capabilities.test.mjs)、各 adapter 原生测试与 A01 conformance；FakeHost 同样拒绝 unknown cancellation |

## 原生旁路范围

逐引擎强制尝试：Codex exec_command/apply_patch/spawn_agent/web.run/browser MCP/connector MCP；Claude Bash/Read/Agent/WebFetch/browser MCP/connector MCP 及被污染的项目 hooks/plugins/MCP；DeepSeek shell/terminal/run_code/delegate/read_file/write_file/browser/web_fetch/外部 MCP。实际负向尝试的权威入口为 [Codex security](../../tests/ai-adapters/codex/security.test.mjs)、[Claude native](../../tests/ai-adapters/claude/native.test.mjs)、[DeepSeek guard](../../tests/ai-adapters/deepseek/guard.test.mjs) / [interactions](../../tests/ai-adapters/deepseek/interactions.test.mjs)。每次最终 CI 都运行这些入口；supported 仅指被断言的关闭/拒绝行为，不从 capability 声明推导隔离保证。

| 入口 | 尝试 / 观察与剩余边界 |
| --- | --- |
| shell、文件、PATH、项目配置、hooks/plugins | 固定真实进程强制原生工具调用与恶意项目/PATH canary，确认未产生预期旁路效果；adapter 固定配置和依赖组合。不能外推任意进程漏洞或同 UID 文件隔离 |
| MCP / IPC | Codex 固定 bridge 的认证/绑定/失效负测；Claude 仅固定 Host MCP，DeepSeek 固定私有 worker IPC 与 proposal。未做同 UID 任意外部 IPC 入侵证明，保留 unknown |
| subagent / browser / connector / 网络工具 | 固定工具目录与被强制调用的原生名称拒绝由各 native suite 断言；共同套件再次检查 conversation 的实际模型工具目录。未覆盖的恶意插件或任意网络系统调用为 unknown |
| 模型 HTTP 外联 | 本地协议服务只证明请求/响应与恢复行为。真实模型 smoke 另跑；模型可访问不等于网络封闭，也不等于受控执行 |
| 动态版本、配置、工作目录、插件组合变化 | 原生 adapter 与 A01 拒绝旧绑定，恢复必须重新准入；回执的 source/lock/platform/profile 不匹配时失效。没有添加可绕过生产 verifier 的测试注册表 |

## 外部真实模型

仅使用明确提供的当前配置，分别运行 `pnpm smoke:codex`、`pnpm smoke:claude`、`pnpm smoke:deepseek`；配置方法见三个 adapter 的 README。缺少凭据或明确 endpoint 时标 unknown/not_run，不搜索个人目录、不用旧回执替代，也不自动改成 fixture。

DeepSeek smoke 在开始时冻结 endpoint/model，实际连接与回执使用同一快照；回执按 endpoint 的真实 origin 区分 official/configured/local_fixture。三个 smoke 均以 `endpointSha256` 记录实际传给 adapter 的完整规范化 URL 摘要（含路径），不保留旧 origin 摘要或明文端点；userinfo/query/fragment 输入被拒绝。official 表示配置指向官方域，`backendIdentityVerified: false` 不允许声称验证了上游模型身份；自定义网关不能写成“官方 DeepSeek 实测”。三个 smoke 均是独立证据，不能升级为生产受控准入或 OS T3。

## 来源与改写

ref: OpenAI Codex `codex-rs/app-server-protocol/src/protocol/v2/mod.rs`、`core/src/session/session.rs`，固定 revision 见 [Codex 来源](../reference/codex-adapter.md)。沿用原生 request/turn terminal，不改原生协议：Codex adapter 按 Host 单次 dispatch 的匹配终态结束观察迭代器，释放单读队列；无 request 坐标的会话观察仍由调用预算结束。六轮真实 Host 用例防止旧观察占用下一轮和 worker 的有界 stream slots。原生接受 turn 后报告 running，Host 的 steer 仅继承目标 run、独立绑定新请求，避免合法 steer 被状态或旧请求坐标拒绝；取消和回答仍保留原请求坐标。

恢复测试与生产应用使用同一个配置/主体/准入 resolver，只在现有 Store.commit 接缝暂停，不给生产 Store/Host 添加故障开关。Rust 编译 helper 被原有丢回执测试与共同受控测试共享，子进程构建设有硬超时。

ref: oh-my-pi [crates/sandbox/src/runner.rs@2f92f3b5aa2035d21c9f016f9ff6350795e5685a](https://github.com/can1357/oh-my-pi/blob/2f92f3b5aa2035d21c9f016f9ff6350795e5685a/crates/sandbox/src/runner.rs#L69)：借鉴 requested/enforced/missing 的证据表达；这里只记录现有 verifier 的要求、断言与缺口，没有移植 sandbox 平台或扩大 A01。

ref: Node.js [lib/internal/url.js@v24.14.1](https://github.com/nodejs/node/blob/v24.14.1/lib/internal/url.js#L879)：`href` 保留规范化路径，`origin` 仅含协议和主机。端点摘要直接复用传入 adapter 的同一 URL 值。参考 [SLSA provenance v1.1 的输入校验](https://slsa.dev/spec/v1.1/provenance#builddefinition)，工具面由实际请求产出并由验收器独立校验，不另建通用 provenance 协议。
