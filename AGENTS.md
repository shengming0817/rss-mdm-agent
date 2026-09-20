# RSS MDM Client / Agent 协作说明

本仓拥有桌面自助、AI引擎适配和客户端受控执行。需求以[产品PRD](docs/product/rss-mdm-agent-prd.md)为准；当前桌面通过 Rust 执行服务与独立 AI Host 装配持久化 S1 闭环，执行器仅为显式测试实现；不能把样本或规划当作真实 OS 执行能力。

- [仓库入口](README.md)与[文档导航](docs/README.md)。
- 稳定规则：[范围](docs/rules/project-scope.md)、[依赖](docs/rules/dependencies.md)、[验证](docs/rules/verification-scope.md)、[文档维护](docs/rules/documentation.md)。
- [Codex工作方式](docs/guides/codex-workflow.md)：项目指令和本地共享技能的边界。

## 工作方式

- 默认中文沟通；修改前读目标文件和相关规则，用`rg`查找已有实现。
- 使用系统Git `/usr/bin/git`，默认集成分支`develop`，通过任务分支/worktree和PR交付；提交遵循Conventional Commits。
- Git/forge操作始终绑定当前目标仓。共享技能、参考源码所在仓不是本次修改或验证目标。
- 只改授权范围；功能/行为变化同步需求与对应文档。禁止`git add -f`提交被忽略的本地内容。
- Azure Boards默认EPIC直接关联PBI，不新增Feature；实施顺序在EPIC评论维护，不复制进度看板到文档。
- 计划和范围授权不等于已实现或已验证；S1测试执行器、真实平台能力、企业接线分别提供证据。
- 创建heartbeat/automation使用UTC；不把用户token、账号目录、本机绝对路径或个人信任设置提交到仓库。

## 产品与来源

执行契约、能力、授权、批准、执行生命周期/存储/服务使用 Rust；AI Host、AI 存储和 provider adapters 使用 Node.js + TypeScript，桌面 UI 使用既定 Vue/Tauri 技术。AI 产品 wire 由独立 JSON Schema 生成 Rust/TS 绑定，禁止双份手写定义。只从prmonitor提取AI引擎与UI，不迁入PR业务或旧全权限/自动批准执行方式。
服务端身份、Group/Scope/Policy/Resource和Agent wire由rss-mdm拥有；按[依赖规则](docs/rules/dependencies.md)消费，不能复制权威或依赖相邻目录path。
来源文件、commit及改写边界记录在提取PR；来源规则不自动成为本仓规则。Linux宿主设计不自动扩大MDM受管平台承诺。

## 验证与协作

- 按[验证规则](docs/rules/verification-scope.md)选择最小有效检查。完整`make ci`一次收集全部失败后集中修复，不逐条修复后反复跑全量。
- 无验证入口时如实记录，不执行父仓CI代替，也不创建无实际消费者的测试/工程空壳。
- 仅在用户或适用技能要求时派子agent，显式`fork_turns`只用`none`、`1`或`2`；按结果协调，不逐步骤指挥或短周期轮询。实现/修复的分工遵循所用技能。
- 问题澄清可使用当前可用的人工输入MCP；工具执行和沙箱批准始终遵循Codex原生机制，不通过消息卡片替代。
- 新建/重构能力查阅一手上游源码并记录`ref: framework file`；技能或工具缺失时先查找和诊断，不默默省略必要步骤。
