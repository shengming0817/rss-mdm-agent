# 验证范围

按变更载体验证，避免用规划、模拟或别的仓库结果替代实际证据。

- 文档/配置：检查语法或结构、相对链接、需求/任务追踪、Git diff、忽略范围和个人信息泄漏；使用本仓实际验证入口，不虚构未运行的检查。
- 核心T1：类型、确定性输入/时钟、未知值、预算、授权与状态转换，覆盖公共API的独立消费。
- 适配T2：使用实际SQLite、协议或provider接缝验证并发、原子性、失败/恢复；缺依赖时不能跳过后报告通过。
- S1共同闭环：测试runner明确标识，实际AI引擎的受控工具旁路另验；fixture不能冒充模型或OS安全证明。
- 后续平台/企业T3：独立限定issue/PR记录OS/架构、主体、版本/产物、原始回执、状态核实与故障范围。

功能实施由实际owner建立本仓本地入口，编辑循环运行受影响的最小有效检查；收尾提交受测源码后运行本仓`make ci CI_BASE=origin/develop`，一次收集全部失败再集中修复。
不运行父仓CI替代产品验证，不新增远端CI。不存在的入口如实说明，不为文档变更引入工程空壳。
证据绑定实际源码SHA、lock、配置/运行模式、命令、结果和未覆盖项；取消、进程退出、安装成功和状态核实分别记录。

## 本地 CI 影响范围

`make ci` 在任务分支比较 `CI_BASE`（默认 `origin/develop`）与受测 HEAD 的 merge-base，
按 Cargo normal/dev/build/optional 与 pnpm dependency/dev/peer/optional 反向依赖闭包选择。
`develop`、detached HEAD、`make ci-full` 或 `CI_FULL=1` 执行全量；Cargo/npm manifest、lock、工具链、
CI 脚本与共享配置、rename/copy、未知路径/删除、缺失基线或分析异常均保守全量。
必须使用 package.json 指定的 Node 与 pnpm 版本。源码身份或基线读取失败不阻止后续 gate 收集，
但最终 provenance 必须失败，不能当作可交付通过证明。

`scripts/ci-impact.mjs` 显式维护 Cargo/pnpm 图外的生成契约、执行绑定、桌面绑定与 runtime
装配边，以及 `tests/` 下公共测试目录的 owner。生成文件变化与源码变化都沿这些关系传播；
不按文件扩展名忽略 crate/package 内可能参与构建的文档。新增 owner、共享 fixture 或脚本时
同步维护该映射；未识别路径全量，不猜测安全排除。

Rust build/test/clippy 使用受影响包；Rust 独立消费者保持整组验证。Node gate 按公共 package
及其消费者选择，先构建选中包的正向生产依赖；类型、格式、边界等共享检查保持保守范围。
无依赖 CI runner 自测先运行，需要 workspace 依赖的产品 harness 在 frozen install 后运行。
纯文档/无变更只运行 CI runner tests、文档/diff 和 committed-source provenance；实际 provider、
原生桌面、凭据与真实 OS 验收仍按各自入口单独运行，选择性 CI 不代替这些证明。

正式 `make ci`/`make ci-full` 强制关闭继承的预览模式。`make ci-plan` 只写 `.local-ci-runs/plan.json`，不执行检查或覆盖正式结果。正式选择写入
`selection.json`，执行结果原子写入 `latest.json`，带选择原因、范围、命令、实际状态和源码身份。
结果区分 passed/failed/skipped；skipped 的 status 为 null，不能当作通过。运行前清理 gate 自有
旧回执与固定 CI runtime，保留手工原生/凭据/smoke 验收和开发 runtime 的独立记录。
所有选中检查执行完才统一返回失败；同阶段不重复全量 CI，只集中修复后精确复验失败项和受影响测试。
