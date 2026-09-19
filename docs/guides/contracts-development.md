# 契约开发与独立消费

独立能力：[execution-contract](../../crates/execution-contract/README.md)、[ai-session-contract](../../crates/ai-session-contract/README.md)。两项基础契约互不依赖；[service-catalog](../../crates/service-catalog/README.md)仅单向消费execution-contract值类型。桌面展示壳当前不接线这些契约。

C10 [脚本计划](../../crates/script-plan/README.md)仅消费 C01 并直接返回 FrozenPlan；C11 [软件计划](../../crates/software-plan/README.md)仅复用 C01 值类型，返回非执行性的单步业务决策。两者纳入同一隔离 consumer 清单，没有桌面或生产 runner 接线。

## 最小检查

执行核心还包括 [C08 批准](../../crates/execution-approval/README.md)与[C09 生命周期](../../crates/execution-lifecycle/README.md)。C08 消费 C07/C01，C09 只消费 C01；各自有真实公共 API example 并纳入独立 consumer 清单。纯核心不交付签发者、数据库或平台执行证据。

```sh
cargo test -p execution-contract -p ai-session-contract -p service-catalog --locked
cargo test -p execution-approval -p execution-lifecycle --locked
cargo test -p script-plan -p software-plan --locked
cargo clippy -p execution-contract -p ai-session-contract -p service-catalog -p script-plan -p software-plan --all-targets --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc -p execution-contract -p ai-session-contract -p service-catalog -p script-plan -p software-plan --no-deps --locked
node --test scripts/boundaries.test.mjs
node --test scripts/rust-consumers.test.mjs scripts/execution-evolution.test.mjs
node scripts/check-rust-consumers.mjs
```

首次准备使用 `pnpm install --frozen-lockfile`；Cargo 的锁定构建取得所需发布包后，独立消费检查以 offline 运行。

consumer 脚本按其明确清单复用各 crate 的真实公共 API example 与 fixture，各自在系统临时目录建立独立 `[workspace]`、lock 和 target。仅允许被测 crate、清单明确声明的本地值类型 owner 与 registry 依赖；核验 metadata 的 workspace/target/source 图，无 Tauri、数据库、provider 或相邻仓依赖。各消费者分别解析依赖并实际运行，不依靠父 workspace feature 统一补齐能力。

独立 consumer 与完整 CI 共用 committed-source 判定：首尾均 clean、HEAD/base/baseRef/baseOid 一致才可报告 PASS；脏树检查仍运行全部 consumer，但总结果标记 failed/non-deliverable。

生成 lock 是隔离源码解析证据，不是 registry 发布或固定 package artifact 验收。实际源码状态、独立 lock 摘要和依赖版本记录在忽略的 `.local-ci-runs/rust-consumers.json`；每轮先使旧结果失效，全部 consumer 分别捕获失败并继续；以原子替换记录本轮 running/passed/failed、命令退出/信号、可得 lock/依赖和清理结果，最后统一非零退出。失败注入测试覆盖旧 PASS、退出码、信号与 spawn 错误；完整 CI 继续收集其它步骤结果。

consumer receipt 与 CLI 保留prepare/isolation的稳定失败码，分别标识缺少owner或registry依赖、workspace/target漂移、意外本地或非registry依赖、运行时依赖越界；附带合法包名，不复制原始异常。Cargo退出码、信号和spawn错误继续单独保留。

## Schema 与交付

上述 Clippy/rustdoc 命令覆盖的五个 crate 均启用 `deny(missing_docs)`，普通编译即检查公共 API 文档；rustdoc 检查同时验证链接/格式。其中 execution-contract、service-catalog 的 schema example 从 Rust 声明输出 Draft 2020-12；AI Runtime V3 改由独立产品 JSON Schema 生成 Rust/TS，旧 AI schema example 已删除；两个计划核心没有新增 schema。只有有意改变当前契约时才更新对应 golden，并审阅真实编码与负向校验；schema 不替代动态预算或可信身份验证。

全部修改提交后执行本仓 `make ci CI_BASE=origin/develop`。入口收集所有失败并记录受测源码，统一修复后复验；不运行 RSS 父仓 CI 替代，也不新增远端 CI。来源映射与设计差异见[契约来源记录](../reference/contracts-extraction.md)。

## AI Runtime V3

[A01 契约包](../../packages/ai-contract/README.md) 持有产品 wire 唯一声明与 TS ports；Rust crate 只承接生成绑定和安全校验。V1 不兼容，不保留转换器或旧 fixtures。共享 fixtures 随 ai-contract/testing 打包，Rust 测试及独立 example 消费同一份。

```sh
pnpm generate:ai-contract
pnpm check:ai-contract
pnpm test:ai-contract
pnpm check:ai-consumer
```

生成工具仅在维护时运行；Rust 独立 consumer 不需要 Node 或相邻 package。TS 隔离 tarball consumer 有独立 package/workspace/lock，从打包产物运行 codec、fake Host 与共用 store conformance。测试替身结果不替代真实 SQLite 事务、模型或旁路隔离证据。完整 CI 绑定 clean committed source；本地快速检查允许脏树，但不能称交付证明。

A01 回调补齐采用直接替换：Interaction 使用必填 `nativeCallbackId` 与 `request`，父请求 ID 仅由 binding/dispatch 持有。消费者同步生成绑定并使用新版 `runStoreConformance`；不保留旧字段兼容。行为映射及同批 pending 投影约束见 [AI Runtime 回调说明](../../packages/ai-contract/README.md#a01-回调契约替换2406)。

Interaction 的必填 `category: "question"` 仅允许普通用户追问；权限 callback 不属于普通 respond 生命周期，进入 ToolEndpoint/verifier 或拒绝。Provider 的 pending 发布只能使用专用 interaction observation。wire schema 按状态闭合：pending 必带 request，answered/expired/unavailable 禁带 request；旧格式直接拒绝，TS/Rust 由同一 schema 生成。

## Provider 恢复准入

新建与恢复都原子返回 `ProviderSessionBinding`。可选 `resume` 显式接收旧 binding、
当前 `ProviderConfiguration` 和 budget，不再返回裸 Binding。Host 使用
`VerifiedProviderSession.resume` 重新验证当前 endpoint、capabilities 和新 generation；
原生会话历史不能继承旧进程的工具准入。适配器与消费方的编译、准入失败清理和真实 SDK
冷恢复测试共同覆盖该接缝，wire schema 本身没有新增字段。
