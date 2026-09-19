# 文档导航

- [客户端 PRD](product/rss-mdm-agent-prd.md)：客户端需求唯一入口，包含执行等级、传统自助与 AI 交互、责任边界和阶段验收。
- [稳定规则](rules/README.md)：范围、依赖、验证与文档维护。
- [Codex工作方式](guides/codex-workflow.md)：项目指令与本地共享技能入口。
- [桌面开发](guides/desktop-development.md)：启动、构建、组件消费、自助固定测试服务与本地验收。
- [UI与桌面壳提取记录](reference/ui-extraction.md)：固定来源、MIT授权和逐文件映射。
- [目录与后端对齐](guides/202609130000-2396-service-catalog.md)：精确资源绑定、参数演进与执行 owner 边界。
- [契约开发](guides/contracts-development.md)：执行/AI 契约、schema 与隔离消费验证。
- [AI Runtime V2](../packages/ai-contract/README.md)：唯一 schema、三个 ports、可靠性和 ACP–A2UI 约定。
- [AI Runtime 来源](reference/ai-runtime.md)：固定上游版本、生成链和许可。
- [契约来源与改写](reference/contracts-extraction.md)：C01/C02 的固定对标及参考重写边界。
- [交互核心](../crates/execution-interaction/README.md)：回答、取消、过期与恢复。
- [能力核心](../crates/execution-capability/README.md)：计划要求与环境快照匹配。
- [授权核心](../crates/execution-admission/README.md)：可信宿主接缝与精确规则。
- [批准核心](../crates/execution-approval/README.md)：完整裁决、可信验证与按尝试消费。
- [生命周期核心](../crates/execution-lifecycle/README.md)：有界快照、证据核实与安全重试。
- [脚本计划核心](../crates/script-plan/README.md)：静态解释器 profile、参数与受控 IO 编译。
- [软件计划核心](../crates/software-plan/README.md)：精确包身份、变更约束与独立检测决策。
- [受控 MCP 适配器](../crates/execution-mcp/README.md)：同源目录参数、宿主绑定服务 port、幂等提交与有界 stdio。
- [执行 SQLite](../crates/execution-sqlite/README.md)与[来源改写](reference/execution-sqlite.md)：原子 journal、交互、批准和结果查询/确认。
- [执行应用服务](../crates/execution-app/README.md)：无 UI 的 S1 组合根、一次性 Test 派发与独立恢复。
- [执行核心来源](reference/execution-cores.md)：固定Rust对标及重写边界。
- [来源与对标](reference/sources.md)：prmonitor 固定源码证据、服务端产品基线及上游参考。

本地文档描述稳定需求与追踪映射。进度、父子关系和滚动波次只在 Azure Boards 的 [EPIC #2392](https://dev.azure.com/shengming0923/rss/_workitems/edit/2392) 维护，不在仓内复制 backlog 状态。
