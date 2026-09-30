# RSS MDM Client / Agent

桌面自助服务与人/AI 共用的受控执行客户端。本仓拥有 UI、AI 适配、本地执行核心与 Agent 平台实现；企业身份、策略、资源发布和 Agent wire 由 rss-mdm 持有。

生产执行由 `apps/agent-service` 统一装配通信、OS 凭据、认证 IPC 和唯一执行 journal。桌面与 AI 引用已验签的后台任务，测试执行器保留在测试装配；后台决定企业授权和来源，客户端校验设备、材料和实际 OS 执行条件。平台验收以 PR 的实际运行记录为准，Windows 编译不等于实机验证。

- [产品需求](docs/product/rss-mdm-agent-prd.md)与[文档导航](docs/README.md)。
- [桌面开发与操作](docs/guides/desktop-development.md)、[安全服务实验室](docs/guides/local-service-lab.md)。
- [协作规则](AGENTS.md)与[来源和许可](docs/reference/sources.md)。
- [EPIC #2392](https://dev.azure.com/shengming0923/rss/_workitems/edit/2392)：任务状态和实施顺序。

默认集成分支为 develop，各产品独立构建和发布。使用根 manifest 与工具链文件指定的工具版本，运行 `pnpm install --frozen-lockfile` 后按桌面指南启动。

本地 `make ci CI_BASE=origin/develop` 按影响范围选择检查，`make ci-full` 强制全量，`make ci-plan` 查看计划。范围和结果语义见[验证规则](docs/rules/verification-scope.md)。
