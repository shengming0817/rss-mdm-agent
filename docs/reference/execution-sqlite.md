# 执行 SQLite 来源与改写

对应 [C18 #2411](https://dev.azure.com/shengming0923/rss/_workitems/edit/2411)。当前实现/API/验证范围由 [execution-sqlite](../../crates/execution-sqlite/README.md) 持有；本页只记录来源。

prmonitor 固定 revision 为 `4dcc87264ad740da6559824e0a8b04a1c2914d4b`。来源仓当时无根 LICENSE；用户于 2026-09-09 明确确认自有项目并授权保持 MIT，记录在[UI 提取权利说明](ui-extraction.md)。目标 Rust 实现按本仓 MIT 授权；不将这一授权表述为来源历史已附 MIT LICENSE，不引入未知第三方片段。

| 固定来源文件 | 查阅机制 | 目标改写与排除 |
| --- | --- | --- |
| [db.rs](https://dev.azure.com/shengming0923/prmonitor/_git/prmonitor?version=GC4dcc87264ad740da6559824e0a8b04a1c2914d4b&path=/src-tauri/src/db.rs) | WAL、pragma 与迁移组织 | database.rs/schema.sql：FULL、立即事务、schema/version 原子提交、只读新版本诊断、严格打开；不提取 PR schema 或旧迁移 |
| [inbox/store.rs](https://dev.azure.com/shengming0923/prmonitor/_git/prmonitor?version=GC4dcc87264ad740da6559824e0a8b04a1c2914d4b&path=/src-tauri/src/inbox/store.rs) | 唯一键接纳与 inbox 持久化 | journal.rs：operationRequestId/规范化内容/主体绑定、保留历史；不提取 webhook/inbox 业务或 retention |
| [outbox/store.rs](https://dev.azure.com/shengming0923/prmonitor/_git/prmonitor?version=GC4dcc87264ad740da6559824e0a8b04a1c2914d4b&path=/src-tauri/src/outbox/store.rs) | 持久结果与状态迁移 | receipts 直接提供查询/拉取/确认；不复制 SELECT-only claim_due、lease、重试 worker 或动作派发 |
| [review/claim_store.rs](https://dev.azure.com/shengming0923/prmonitor/_git/prmonitor?version=GC4dcc87264ad740da6559824e0a8b04a1c2914d4b&path=/src-tauri/src/review/claim_store.rs) | 单次领取/条件状态写入 | execution.rs/trust.rs：受保护事务中的 attempt/event 历史唯一、批准计数 CAS；不带 PR/head/skill 维度 |
| [lib.rs::process_rule_event](https://dev.azure.com/shengming0923/prmonitor/_git/prmonitor?version=GC4dcc87264ad740da6559824e0a8b04a1c2914d4b&path=/src-tauri/src/lib.rs) | 状态与后续任务同事务写入 | 状态/批准/intent/审计/回执一同提交，动作仅由 C09 首次 commit 释放；不复制规则调度/外部动作自动重试 |

以上为针对机制的参考重写，没有整体复制来源模块。来源的进程 mutex 与 deferred transaction 不能替代跨连接写序列化；SELECT-only claim 不提供持久排他；删除幂等载体的保留策略不适合一次性批准；迁移多语句必须连同版本号一起回滚。C18 使用自身 schema/命令 API，不保留来源数据库或业务 API 兼容。

直接 Rust 对标读取 [rusqlite src/transaction.rs](https://github.com/rusqlite/rusqlite/blob/499cc7bb986e04cc66e6ed762522f7ea449178d1/src/transaction.rs)，固定 0.32.1 revision `499cc7bb986e04cc66e6ed762522f7ea449178d1`；使用其 Immediate/commit/Drop 回滚语义，无源码复制。Cargo.lock 固定 rusqlite 0.32.1（MIT）与 libsqlite3-sys 0.30.1（MIT，bundled SQLite 公有领域），无 SQLite 系统安装前提。WAL/FULL、写锁、冲突与容量行为另核对 SQLite 官方 [事务](https://www.sqlite.org/lang_transaction.html)、[synchronous](https://www.sqlite.org/pragma.html#pragma_synchronous)、[UPSERT](https://www.sqlite.org/lang_upsert.html)、[max_page_count](https://www.sqlite.org/pragma.html#pragma_max_page_count) 文档，并以实际文件故障测试验证本 adapter。
