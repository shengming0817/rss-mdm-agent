# 来源与对标

本文件记录需求与源码证据，不作为实现状态或依赖选型批准。初始查阅日期：2026-09-09 UTC；AI Runtime 契约核对：2026-09-18 UTC；provider 基线核对：2026-09-19 UTC。
PRD唯一入口为[客户端PRD](../product/rss-mdm-agent-prd.md)。D00仅编写文档；C05的实际UI/桌面壳提取、权利授权和固定对标见[提取记录](ui-extraction.md)。

C01/C02 的实际契约实现及固定来源映射见[契约来源记录](contracts-extraction.md)。C18 的 SQLite 机制参考重写见[执行存储来源](execution-sqlite.md)，不提取 inbox/outbox 的 PR 业务。

## 固定产品来源

| 来源 | 固定身份 | 用途 |
| --- | --- | --- |
| 用户确认的客户端方案 | [EPIC #2392](https://dev.azure.com/shengming0923/rss/_workitems/edit/2392)、[D00 #2393](https://dev.azure.com/shengming0923/rss/_workitems/edit/2393) | 人工自助与AI共用受控执行、只提取AI/UI、20项功能与本批文档交付 |
| rss-mdm-agent原始基线 | `f423420236804a468b7c7fc54fce38f65ea7710c` | 空初始仓；不是已实现Agent |
| rss-mdm产品基线 | `589211a598d588508375f7e84742cfa0dc7d29ce` | 产品范围、仓库owner、协议producer与T3边界 |
| prmonitor提取来源 | `4dcc87264ad740da6559824e0a8b04a1c2914d4b` | AI引擎/UI源码参考；实际提取范围以逐文件记录为准 |

服务端[工程目标](https://dev.azure.com/shengming0923/rss/_git/rss-mdm?version=GC589211a598d588508375f7e84742cfa0dc7d29ce&path=/docs/product/project-goals.md)、[产品PRD](https://dev.azure.com/shengming0923/rss/_git/rss-mdm?version=GC589211a598d588508375f7e84742cfa0dc7d29ce&path=/docs/product/rss-mdm-prd.md)、[架构ADR](https://dev.azure.com/shengming0923/rss/_git/rss-mdm?version=GC589211a598d588508375f7e84742cfa0dc7d29ce&path=/docs/architecture/adr/202609072231-001-rust-rss-product-foundation.md)。
本地服务端仓包含WinMDM历史快照，恢复来源由其reference/README.md持有；历史实现不是本客户端交付证明。

## Provider 来源

实际提取与改写分别见 [Codex](codex-adapter.md)、[DeepSeek](deepseek-harness.md)与 [AI Runtime](ai-runtime.md)。依赖版本以 manifest 和 lock 为准，来源 revision 仅用于追溯。

## prmonitor 源码提取清单

以下链接固定上述commit。实际复制时必须记录新增/删除/改写与许可证/权利依据；当前根目录未发现LICENSE，不臆造开源许可，也不把第三方代码权利等同于仓库所有权。

| 源码 | 可参考的能力 | 必须移除/改写 |
| --- | --- | --- |
| [review/engine.rs](https://dev.azure.com/shengming0923/prmonitor/_git/prmonitor?version=GC4dcc87264ad740da6559824e0a8b04a1c2914d4b&path=/src-tauri/src/review/engine.rs) | engine trait、启动/续聊结果、能力化入口模式 | pr_number、SkillInvocation、PR/skill去重、Review身份 |
| [Codex adapter](https://dev.azure.com/shengming0923/prmonitor/_git/prmonitor?version=GC4dcc87264ad740da6559824e0a8b04a1c2914d4b&path=/src-tauri/src/review/engines/codex) | RPC、codec、进程/流式会话 | review prompt、自动批准与全权限默认；按实际版本验证工具控制 |
| [Claude adapter](https://dev.azure.com/shengming0923/prmonitor/_git/prmonitor?version=GC4dcc87264ad740da6559824e0a8b04a1c2914d4b&path=/src-tauri/src/review/engines/claude) | 流事件、CLI会话、resume差异 | bypassPermissions、PR skill入口 |
| [历史 Cursor adapter](https://dev.azure.com/shengming0923/prmonitor/_git/prmonitor?version=GC4dcc87264ad740da6559824e0a8b04a1c2914d4b&path=/src-tauri/src/review/engines/cursor) | 仅历史 ACP流与进程generation 参考，不是当前 provider 实施来源 | 当前范围移除；不得继承 force/sandbox-disabled、反向自动批准或PR上下文 |
| [review/session.rs](https://dev.azure.com/shengming0923/prmonitor/_git/prmonitor?version=GC4dcc87264ad740da6559824e0a8b04a1c2914d4b&path=/src-tauri/src/review/session.rs) | 会话事件与UI连接的来源线索 | approval_policy=never、DangerFullAccess、PR完成/评论绑定；不整体提取 |
| [ReviewStream.vue](https://dev.azure.com/shengming0923/prmonitor/_git/prmonitor?version=GC4dcc87264ad740da6559824e0a8b04a1c2914d4b&path=/src/review/ReviewStream.vue)、[SplitPane.vue](https://dev.azure.com/shengming0923/prmonitor/_git/prmonitor?version=GC4dcc87264ad740da6559824e0a8b04a1c2914d4b&path=/src/SplitPane.vue) | 消息展示、流式交互与布局 | review store/API、PR字段、宿主强耦合 |
| [types.ts](https://dev.azure.com/shengming0923/prmonitor/_git/prmonitor?version=GC4dcc87264ad740da6559824e0a8b04a1c2914d4b&path=/src/types.ts) | 类型化事件的组织方式 | 不整体复制PullRequest/Workflow等PR契约 |

排除 src-tauri/src/pr、src/pr、PR 调度/标签/评论/webhook 业务、PR 数据库、remote terminal、messaging 及其配置。A02 可参考 inbox/outbox 的事务、内容绑定和故障语义，在 TS AI 模型中重写；不复制 PR schema、历史 migration 或整套 worker。独立执行SQLite/MCP/交互能力在目标仓按新契约实现，不能借来源已有模块扩大提取范围。

## 直接上游对标

下列均为一手项目/官方文档。文档参考链接可能随main演进；实现PR必须再固定所消费版本、commit与许可证，不能直接将main当生产依赖。

| 上游 | 参考内容 | 使用边界 |
| --- | --- | --- |
| [Mullvad架构](https://github.com/mullvad/mullvadvpn-app/blob/main/docs/architecture.md) | GUI/CLI与后台daemon职责分离 | 参考模式，不复制VPN/产品权威；具体源码许可分别核对 |
| [windows-service](https://github.com/mullvad/windows-service-rs/blob/main/src/service_dispatcher.rs) | Rust服务入口与控制事件 | 后续Windows adapter候选，不是提权授权引擎 |
| [service-manager](https://github.com/chipsenkbeil/service-manager-rs/blob/main/src/lib.rs) | launchd/systemd/sc等服务管理接口 | 后续平台服务候选，不能证明完整设备管理 |
| [process-wrap](https://github.com/watchexec/process-wrap/blob/main/src/tokio.rs) | 进程组/session/Job Object组合 | 进程生命周期机制，不等于安全沙箱或副作用回滚 |
| [Goose权限源码](https://github.com/aaif-goose/goose/blob/main/crates/goose/src/config/permission.rs) | 工具权限配置与确认模型 | 产品授权由可信代码强制，不能把模型风险判断当authority |
| [官方Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk) | rmcp工具/协议适配 | MCP是入口，不提供产品级操作授权或OS隔离 |
| [MCP工具注解说明](https://blog.modelcontextprotocol.io/posts/2026-03-16-tool-annotations/) | 注解与强制安全契约的区别 | readOnlyHint等不构成放行依据 |
| [Cedar Rust API](https://github.com/cedar-policy/cedar/blob/main/cedar-policy/src/api.rs) | 主体/动作/资源/上下文判定 | 可选provider，不以本PR引入通用IAM或强制策略语言 |
| [Wasmtime安全模型](https://docs.wasmtime.dev/security.html) | WASI能力式访问 | 后续可选受限计算，不使宿主原生脚本自动沙箱化 |

## 平台限制的一手依据

- [Windows服务与用户交互](https://learn.microsoft.com/en-us/windows/win32/services/interactive-services)：系统服务与用户会话分离。
- [Apple daemon/agent](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/CreatingLaunchdJobs.html)：系统daemon与用户agent职责。
- [PowerShell执行策略](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_execution_policies)：ExecutionPolicy不是安全边界。
- [WinGet System Context](https://learn.microsoft.com/en-us/windows/package-manager/winget/troubleshooting)：CLI不支持LocalSystem，官方PowerShell client对机器级应用的路径须实际验证。
- [Homebrew FAQ](https://docs.brew.sh/FAQ)：非root与单用户安装模型，不能由root daemon直接替代获准用户操作。
- [systemd执行配置](https://github.com/systemd/systemd/blob/main/man/systemd.exec.xml)：权限/资源/运行用户约束由具体平台强制。
- [Landlock兼容性](https://landlock.io/rust-landlock/landlock/trait.Compatible.html)：区分best-effort和硬要求，产品不能静默放弃必需限制。

这些来源支持设计取舍，不构成Windows/macOS/Linux任何具体版本的本产品支持证明。

AI Runtime `ai-runtime-20260918` 的单一 schema、TS/Rust 职责、固定上游及许可记录见 [A01 来源](ai-runtime.md)。Cursor 历史源码仍是来源记录，#2407 已取消，不构成现行 provider 承诺。

## 服务目录对齐


本次为新实现，不提取 prmonitor AI/UI，也不复制历史后端源码；不继承其运行权限或协作规则。

- rss-mdm 对齐观察 revision：`6f3236dfc80cc50f44d5980f4e584e7670374021`。已读取 [Resource 定义与解析](https://dev.azure.com/shengming0923/rss/_git/rss-mdm?path=/crates/resource/src/lib.rs&version=GC6f3236dfc80cc50f44d5980f4e584e7670374021) 的 Artifact（97–127行）、Version（232–301行）与 canonical（303行起）。这些是语义证据，不是已发布跨仓协议。
- ref: schemars `schemars/src/generate.rs@v1.2.2`，已读取 [Draft 2020-12 与 schema 生成](https://github.com/GREsau/schemars/blob/v1.2.2/schemars/src/generate.rs#L99-L110)。目录文档 schema 从 Rust 声明生成；动态参数由闭合规则同源投影。秘密引用内联其 owner schema，避免嵌套 schema 的根引用错位。
- ref: jsonschema `src/lib.rs@0.33.0`，已读取 [验证与禁止外部引用解析的 feature 配置](https://docs.rs/jsonschema/0.33.0/src/jsonschema/lib.rs.html#226-262)。只用于测试，关闭默认 features；核心不运行任意外来 schema，也不访问远程引用。
- [JSON Schema Draft 2020-12 validation](https://json-schema.org/draft/2020-12/json-schema-validation#section-9.2)：default 是注解，目录通过共享 runtime 明确补值；业务整数另受安全范围约束。
