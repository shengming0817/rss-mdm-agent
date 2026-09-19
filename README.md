# RSS MDM Client / Agent

面向 Windows、macOS、Linux 宿主设计的桌面自助服务与人/AI 统一受控执行客户端。
本仓拥有客户端 UI、AI 引擎适配、本地执行核心与后续 Agent 平台实现；服务端产品与 Agent wire 由 rss-mdm 拥有。

当前包含独立自助目录核心、执行/AI 会话契约、Vue UI 包、Tauri 桌面自助页面与同导航 AI 助手。自助页面通过受限 IPC 消费 Rust 内存固定测试服务，普通浏览器入口提供只读自助预览；助手未注入真实服务时显示未连接，独立固定 Host 验收复用同一产品页面。尚无正式后台装配或安装包。本批 C01–C20 的目标是独立核心、从 prmonitor 提取 AI/UI 和测试执行器闭环；真实平台命令、软件安装、提权、远程管理与 T3 留后续交付。

- [产品 PRD](docs/product/rss-mdm-agent-prd.md)：需求、范围、验收和任务追踪。
- [协作入口](AGENTS.md)与[Codex工作方式](docs/guides/codex-workflow.md)。
- [文档导航](docs/README.md)：文档唯一入口。
- [AI 助手开发与验收](docs/guides/assistant-development.md)：共享导航、固定 Host、多窗口交互与 Rust 授权执行详情。
- [来源与对标](docs/reference/sources.md)：固定来源和选择边界。
- [自助目录核心](crates/service-catalog/README.md)：人/AI同源参数、精确选择与非授权状态说明。
- [脚本计划](crates/script-plan/README.md)与[软件计划](crates/software-plan/README.md)：原生脚本编译和单步软件核实决策。
- [批准核心](crates/execution-approval/README.md)与[生命周期核心](crates/execution-lifecycle/README.md)：精确批准消费意图、取消/退出/核实与有界恢复。
- [AI SQLite](packages/ai-store-sqlite/README.md)：产品会话持久化、恢复与单 Host 独占；真实 AI Host/引擎接线另行交付。
- [执行 SQLite](crates/execution-sqlite/README.md)：受保护原子接纳、批准消费、幂等回执与可靠结果；显式测试 authority。
- [无 UI 执行应用服务](crates/execution-app/README.md)：能力/准入/审批/生命周期组装、Test runner、恢复与配置接缝；尚未接入桌面/AI 或生产 runner。
- [契约开发与独立消费](docs/guides/contracts-development.md)：本地执行摘要与 AI 工具提案。
- [客户端 EPIC #2392](https://dev.azure.com/shengming0923/rss/_workitems/edit/2392)：任务状态和最新 `pm:epic-wave` 实施顺序真源。

prmonitor 只提供 AI 引擎与通用 UI 的提取来源，新客户端不包含 PR 业务。
本仓的 Linux 宿主设计不自动扩大 rss-mdm 当前 Windows/macOS 企业受管平台承诺。

默认集成分支为 `develop`。各产品独立发布，禁止依赖相邻仓目录才能构建；工程使用根级 Cargo/pnpm workspace，入口与验证说明见[桌面开发指南](docs/guides/desktop-development.md)。
