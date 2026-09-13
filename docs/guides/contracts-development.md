# 契约开发与独立消费

两项独立能力：[execution-contract](../../crates/execution-contract/README.md)、[ai-session-contract](../../crates/ai-session-contract/README.md)。它们互不依赖，桌面展示壳当前不接线这些契约。

## 最小检查

```sh
cargo test -p execution-contract -p ai-session-contract --locked
cargo clippy -p execution-contract -p ai-session-contract --all-targets --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc -p execution-contract -p ai-session-contract --no-deps --locked
node --test scripts/boundaries.test.mjs
node scripts/check-rust-consumers.mjs
```

首次准备使用 `pnpm install --frozen-lockfile`；Cargo 的锁定构建取得所需发布包后，独立消费检查以 offline 运行。

唯一Rust consumer脚本同时验证两契约与三个执行核心，复用真实公共API example和fixture，各自隔离workspace、lock和target；源码依赖按明确清单限制，完整规则见[执行核心开发说明](execution-cores.md)。

独立 consumer 与完整 CI 共用 committed-source 判定：首尾均 clean、HEAD/base/baseRef 一致才可报告 PASS；脏树检查仍运行全部 consumer，但总结果标记 failed/non-deliverable。

生成 lock 是隔离源码解析证据，不是 registry 发布或固定 package artifact 验收。实际源码状态、独立 lock 摘要和依赖版本记录在忽略的 `.local-ci-runs/rust-consumers.json`；每轮先使旧结果失效，全部 consumer 分别捕获失败并继续；以原子替换记录本轮 running/passed/failed、命令退出/信号、可得 lock/依赖和清理结果，最后统一非零退出。失败注入测试覆盖旧 PASS、退出码、信号与 spawn 错误；完整 CI 继续收集其它步骤结果。

## Schema 与交付

两个 crate 均启用 `deny(missing_docs)`，普通编译即检查公共 API 文档；rustdoc 检查同时验证链接/格式。两个 schema example 从 Rust 声明输出 Draft 2020-12。只有有意改变当前契约时才更新对应 golden，并审阅真实编码与负向校验；schema 不替代动态预算或可信身份验证。

全部修改提交后执行本仓 `make ci CI_BASE=origin/develop`。入口收集所有失败并记录受测源码，统一修复后复验；不运行 RSS 父仓 CI 替代，也不新增远端 CI。来源映射与设计差异见[契约来源记录](../reference/contracts-extraction.md)。
