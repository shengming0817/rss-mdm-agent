# DeepSeek Harness 来源与验证

A05 #2443 直接消费 DeepSeek Harness `0.1.6-alpha.2` 的 npm 产物；研究源码固定为
[`ddefc45fbc7f8e46dd73185e68295696d1297887`](https://github.com/deepseek-ai/deepseek-harness/tree/ddefc45fbc7f8e46dd73185e68295696d1297887)。源码不作为运行时相邻目录依赖；完整解析闭包及 npm integrity 由本仓 `pnpm-lock.yaml` 持有。发布包中的 generated Typert 描述是 Gateway 的运行输入，未以源码装饰器代替。

上游 MIT 许可，保留各 npm 包的许可文件；本适配器是新实现，未复制整套上游 Agent、工具调度或持久化算法。`support.ts` 沿用本仓 Claude adapter 的 MIT 有界等待/队列辅助实现；未迁移 prmonitor 的 PR 业务或全权限配置。

| 一手来源（上述固定 revision 下） | 消费/改写边界 |
| --- | --- |
| `packages/api/gateway/src/index.ts`、发布 `dsh-api-session-controller/typert` | 直接消费 Gateway unary/stream 与生成描述；私有 IPC 只映射必要固定操作 |
| `packages/api/session-controller/src/{index,commands,history,agent}.ts` | 消费 create/prompt/cancel/follow/inspect；restore 只 inspect，后续显式 submit 激活 |
| `packages/core/agent-loop/src/{inbox,agent,assistant-stream}.ts` | 原生 requestId 写入 user source.rpcId；适配器通过 inbox claim/turn 关联历史，不重写原生 inbox |
| `packages/session/session-checkpoint-policy/src/index.ts` | 直接加载发布 checkpoint policy，模型/工具前等待 flush |
| `packages/session-query/session-query/src/cold-read.ts` | 冷读 synthetic interrupted 不视为耐久终态 |
| `packages/core/tools/src/index.ts` | 直接消费 monotonic deny guard；固定 profile 与工具定义，禁用 PTC/委派 |
| `packages/interaction/{user-questions,tool-ask-user}/src/index.ts` | 保留原生结构化问题；改写为 A01 generation-bound callback，不向回答赋予执行权 |
| `packages/llm/llm-deepseek/src/{adapter,config}.ts` | 固定官方 chat-completions；关闭 Harness 元数据扩展、模型默认 thinking，密钥从可信 resolver 内存传递 |

本适配器的 TS 实现仍严格检查；仅该包 `skipLibCheck=true` 跳过上游产物声明的已知不兼容（cosmokit 泛型 typed arrays、SessionFormat 索引签名及 optional jobs 类型引用）。公共入口不暴露这些原生类型；隔离 consumer 用 `skipLibCheck=false` 验证真实公共类型闭包。

验证分开记录：

- A01 fixture：提交确定性、attempt/correlation 绑定、旧 observer 拒绝、一次性准入、迟到初始化和 cleanupError。它不证明模型或设备效果。
- 真实 Harness + 本地 HTTP 模型协议：真实 delta、稳定文本、JSONL、完整 restore/rebind/reconcile/commit、冷续接上下文、模型/工具 flush、崩溃窗口、交互/取消、权限负测与 guard 单调性。恢复期间比对原生日志字节和模型请求数。
- 官方 DeepSeek：显式 smoke 使用实际凭据验证增量、历史和冷上下文续接；记录平台、模型、源码状态、lock hash、组合版本。测试工具结果只声明 S1；不能改写 Rust Evidence。
- 固定 artifact：打包 contract/adapter，临时独立 workspace 用自身 lock 安装，公共类型及真实子进程 cold session 通过；记录 archive SHA-256 与 consumer lock SHA-256，禁止回指本仓源码。

这些组件证据不承诺生产 Host/A02 存储实现、操作系统 sandbox、Windows 实测或企业接线。Host 必须持有可信 namespace、平台 verifier、原子状态转移与恢复凭证流程。未知提交不能自动重发；即使日志没有对应请求也不能证明未发送。
