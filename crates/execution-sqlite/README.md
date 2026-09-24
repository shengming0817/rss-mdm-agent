# execution-sqlite

将执行、交互、批准消费与可靠结果放入同一受保护 SQLite authority。当前只提供 Test authority 初始化，AI ledger 与生产平台身份分别由各自 owner 持有。

存储读取当前状态并调用核心，不接受调用方自造快照或批准计数。原子事务同时复核授权新鲜度与精确绑定，提交批准消费、intent、状态、审计和回执；只有首次提交释放派发动作。重放读取原回执，未知结果转核实，不再造执行。

可信 Host 回调须有界、非重入且不能访问网络。执行、交互、结果投递与审计权限分离；声明 scope 不授予权限。可靠结果按消费方持久确认推进，乱序确认不能跳过未确认记录，消费方先持久处理再确认。

## 文件、容量与恢复

initialize_test 在同一私有目录的独立临时文件中完成迁移、checkpoint 和关闭，再通过不覆盖 hard link 原子发布全新数据库；中断只留下未发布的隔离文件，显式重试不会读取或接管它。已发布数据库只通过 open 恢复。废弃的 .execution-bootstrap-* 及其 sidecar 由管理员在确认没有初始化任务后清理；当前没有自动清理器。该发布要求所在文件系统支持同目录 hard link。初始化要求绝对规范路径和预建私有目录；open 只打开已有库。Unix 检查目录/数据库/WAL/SHM/journal 无组或其他用户权限、拒绝符号链接，文件创建为 0600。WAL + synchronous=FULL + foreign_keys=ON，macOS 同时启用 fullfsync/checkpoint_fullfsync；busy wait 显式有界。没有损坏重建、备用路径或隐式 authority 替换。Windows ACL 与真实 AI/用户进程隔离属于后续平台接线，不由文件 mode 测试代替。

只接受当前持久格式；版本或完整性不支持时保留原文件并失败，不提供兼容读取、自动重建或历史导入。已打开连接仍须复核 authority 与格式。

容量受显式预算约束，读取 BLOB 前先核对存储类型和长度。为终止与核实保留逻辑容量，但它不等于磁盘预分配。存储失败不自动触发业务重试。错误按恢复动作区分：

| 错误 | 调用方处置 |
| --- | --- |
| InvalidInput | 修正 operation ID、拉取参数或新聚合输入；不重配 store |
| Configuration | 修正显式 Limits 或 SQLite 持久化配置 |
| OperationCommitUnknown | 查询或重交同一 operationRequestId；绝不能换 ID 重跑 runner |
| ConfirmationCommitUnknown | 以同一 scope/consumer/event 重试 confirm；它没有 operation receipt |
| BootstrapUnpublished | 初始化 migration 提交失败，目标库未发布；诊断存储或显式重试同一路径的 initialize_test |

BootstrapUnpublished 仅来自发布前的 migration 提交；其它文件系统错误仍为 Storage。若初始化在发布后发生错误，保留目标文件并通过 open 核查，不能覆盖或重建 authority。


API 与配置见 [src](src/)，来源和权利依据见[来源记录](../../docs/reference/execution-sqlite.md)。
