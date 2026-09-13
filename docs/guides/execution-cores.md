# 执行核心开发与独立消费

本批交付[C04交互](../../crates/execution-interaction/README.md)、[C06能力](../../crates/execution-capability/README.md)、[C07授权](../../crates/execution-admission/README.md)；三者互不依赖，能力与授权消费execution-contract。没有SQLite、UI、模型或平台执行接线。

## 执行契约原子切换

PlanSpec新增必填`sessionRequirement`：`notRequired`或带明确account的`activeUser`。账号平台必须匹配target；它与发起人OS登录独立，进入规范编码、计划摘要、能力匹配和授权规则。旧计划缺少该字段直接拒绝，不补默认值、不提供旧reader。当前未发布本地V1契约直接切换，所有示例、schema、摘要及审计关联fixture同步更新；这不是Agent wire变更，也没有生产数据迁移承诺。

## 最小验证

```sh
cargo test -p execution-contract -p execution-interaction -p execution-capability -p execution-admission --locked
cargo clippy -p execution-contract -p execution-interaction -p execution-capability -p execution-admission --all-targets --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc -p execution-contract -p execution-interaction -p execution-capability -p execution-admission --no-deps --locked
node --test scripts/rust-consumers.test.mjs
node scripts/check-rust-consumers.mjs
```

单个crate的示例验证公共API；`check-rust-consumers.mjs`是唯一Rust独立消费入口，一份明确清单覆盖execution-contract、ai-session-contract与三个新核心。每个consumer独立workspace/lock/target，依赖关闭default features，检查metadata及源码依赖边界并实际运行。能力/授权consumer显式依赖execution-contract，不利用其它workspace成员补齐依赖。工具失败逐项收集，旧成功记录先失效，临时目录逐项清理。

仅被测crate与清单声明的execution-contract可作为源码依赖，其余来自registry；禁止相邻产品仓、Tauri、数据库与模型provider依赖。源码/配置/锁的实际身份和结果记录于忽略的`.local-ci-runs/rust-consumers.json`。验证开始与结束必须是同一clean committed HEAD/base才可报告交付PASS；编辑期脏树的consumer即使运行成功，总结果仍不可交付。

## 验证含义

交互竞争是纯转换与revision条件提交的T1，真实数据库CAS归C18。能力快照与授权示例明确为固定测试输入，没有真实OS探测、身份验证或沙箱证明。可信verifier接口不证明生产adapter已经实现；C19须验证身份、来源、撤销、时间和政策接线，并强制所有执行门。

全部源码提交后运行本仓完整`make ci CI_BASE=origin/develop`，一次收集失败后集中修复。验证绑定最终源码SHA，不运行父仓CI、不新增远端CI。固定上游参考见[来源记录](../reference/execution-cores.md)。
