# A02 SQLite 来源与改写

对应 #2440，范围基线 `ai-runtime-20260918`。本包源码与 SQL 为本仓重新实现，MIT 许可；没有复制历史数据库、migration、PR 业务或原生引擎 transcript。公共领域规则由 ai-contract 的 `/transitions` 唯一持有。

| 固定来源 | 采用的机制 | 明确改写/不采用 |
| --- | --- | --- |
| [Pi sqlite-node/index.ts](https://github.com/earendil-works/pi/blob/d981de1229ef899957bbe968bc8dcda02a21f477/packages/session-backends/sqlite-node/src/index.ts) | 同步 SQLite 调用、BEGIN IMMEDIATE、失败回滚、消费 conformance | 自建本产品表和完整 namespace；用长连接 EXCLUSIVE/WAL 明确单 Host 所有权，不继承外部自行串行化的隐含前提 |
| prmonitor `4dcc87264ad740da6559824e0a8b04a1c2914d4b`：`src-tauri/src/db.rs`、`inbox/store.rs`、`outbox/store.rs`、`lib.rs::process_rule_event` | durable accept、内容绑定、同事务写出事件及回滚测试 | 不复制 Rust sidecar/PR schema。原 claim_due 只是 pending 查询，没有持久 claim/lease；本包也不宣称多 worker。producer key 的保留由本产品 receipt 与永久 namespace tombstone 重新定义 |

prmonitor 固定来源及恢复方式由[来源索引](sources.md)持有，前置探索直接核对了上述源码。逐文件目标：`src/schema.ts` 拥有独立 SQL/版本/checksum；`src/index.ts` 拥有 Node 数据库适配与文件/连接生命周期；`tests/ai-store-recovery` 为本产品事务与进程故障验收。没有原样源码复制文件，故不引入来源运行依赖或另附源码许可副本。

官方机制依据：

- [Node 24.14.1 node:sqlite](https://nodejs.org/download/release/v24.14.1/docs/api/sqlite.html)：使用内置 DatabaseSync，运行时实际验证 SQLite3.51.2；API 仍标 experimental。
- [SQLite locking_mode](https://sqlite.org/pragma.html#pragma_locking_mode) 与 [无共享内存的 WAL](https://sqlite.org/wal.html#use_of_wal_without_shared_memory)：在 WAL 前启用 EXCLUSIVE，首次真实写入后持锁至连接关闭。
- [SQLite transactions](https://sqlite.org/lang_transaction.html)：同步事务、COMMIT 失败和回滚；短时写锁自身不等于 Host 所有权。
- [SQLite synchronous](https://sqlite.org/pragma.html#pragma_synchronous)：WAL/FULL 的同步屏障。本次测试使用进程 SIGKILL，不声称已模拟真实断电。

这些链接是机制与版本来源；实际本产品验证结果在 PR 与 `.local-ci-runs` 的可再生、绑定源码 SHA 的记录中，不把上游行为说明冒充本产品验收。
