# execution-sqlite

C18 将执行、交互、批准消费和可靠结果放进同一受保护 SQLite authority。只依赖 C01/C04/C07/C08/C09 与基础库；没有 AI ledger、Node 写库入口、runner、消息 worker 或 PR 业务。C19 实现 Host 并组合 runner；当前仅提供显式 Test authority 初始化。

## 公共入口与事务

Store 接收冻结计划、生命周期事件、交互命令与 operationRequestId，自己读取当前状态并调用核心。调用方不能提交自造 Snapshot、Transition、审计或批准计数。所有写入使用 BEGIN IMMEDIATE；SQLite 负责不同连接的竞争，进程内 mutex 不作为正确性条件。

`apply_command(op, scope, CommandEvent, bindings, host)` 提交普通命令；`apply_observation(op, scope, ObservationEvent, host, verifier)` 提交观察，始终要求 RunnerFact 权限。两入口共用历史去重、审计、CAS 与提交流程，类型上禁止混用；Host 无需实现 ObservationVerifier。操作指纹包含 EventRecord 与批准引用，验证结果不参与指纹；重放不重新解析证据。

每个新操作先在事务内查询 operationRequestId。相同 scope/规范化命令返回原 Receipt；同 ID 不同内容或主体冲突。重放允许原动作权限或独立 ReadResult 权限读取安全回执，不重新申请执行批准、消耗次数或生成派发动作。操作、事件、交互命令与 attempt 历史键保留整个 authority 生命周期，没有删除幂等记录或导入旧库的 API。业务拒绝、Stale、Late、NotDue 等结果也有稳定回执；存储失败与未可信输入返回封闭错误码，不落半份业务结果。

BeginAttempt 在同一事务内读取受保护计划、authority/批准修订和消费计数，通过 Host 获取完整 C07/C08 裁决，再复核可靠时间、精确绑定、版本、有效期、历史唯一键和 CAS。批准消费、Starting intent、状态、审计和结果同时提交。完整 C07/C08 裁决、规则 ID、profile→record 映射、时效、每份批准及消费前后计数单独保存在有特权读取权限的 AuditRecord。拒绝路径也保留提交引用与受保护记录，二者明确区分；AuditReason 使用闭集枚举和明确的 serde 映射，不持久化 Debug 文本。批准定义按 record/version 不可变；刷新不接受 used/revision，不能恢复次数。携带 attempt 的命令在校验前保存提交的 attempt；成功、拒绝、陈旧及回执重放均保持该关联，Receipt 从 AuditRecord 的同一字段派生。陈旧/重复核心结果同样持久化 lifecycle directive；它只表示下一步建议。

只有生命周期 Transition::commit 的首次成功提交能返回 DispatchAction。它不可复制、不可反序列化且消费一次；提交响应丢失、丢弃动作或重启都不能再造动作。Starting 只证明 intent；恢复给出 Reconcile，显式 Recover 写为 Unknown，绝不从日志重放执行。取消、退出和状态核实仍分别由 C09 表达。

## Host 与读取授权

Host 是可信产品代码，不是 UI/模型 DTO。authorize 独立验证实际 caller、authority、actor、plan、consumer；Interact 还验证 responder、命令和回答引用。admit 使用同事务提供的 ApprovalVerifier，返回完整 AdmissionDecision 与 ApprovalDecision；无批准路径同样检查当前 authority revision。trusted_snapshot 验证签发权限、来源与撤销，返回完整快照；缺失记录不可用。可靠时钟不得回拨至已提交 watermark 之前；事务内回调须有界、不重入、不访问网络。

Scope 来自完整 authority/actor/plan，包含企业 authority 的 tenant；声明 scope 不赋予权限。ReadResult、ReadAudit、Deliver 与 Execute 分离。没有正确 Host 的初始化、反序列化或查询不能赋予执行权限；本 crate 不防御恶意同进程 Host、本机管理员或内核失陷。

`execution_by_request` 要求封闭 `ExecutionAccess`，Delivery 必须携带 consumer；解析出的完整 scope 仍由 Host 认证，不隐含 ReadResult。返回的 `ExecutionRecord` 在同一事务内包含 C09 状态与最近非陈旧准入投影（Admitted/Denied/ApprovalRequired）；投影来自原 receipt，不泄露审计详情，不新增表。`execution_receipt` 仅供 Execute 权限恢复执行命令，其他回执查询仍使用对应授权入口。stop 响应保留 attempt 和闭集 Acknowledged/Failed 审计，不代替终止/效果事实，也不占用终态证据预留额度。

Receipt 同时作为可靠结果，不另建 outbox。pull_results(scope, consumer, limit) 按 sequence 返回该 consumer 最早的未确认结果，没有调用方游标；sequence 只用于排序，只有持久确认才推进投递。乱序/部分确认和重启不会跳过较早未确认记录；响应丢失直接重拉，业务方先持久化自己的处理结果再 confirm。确认幂等且不产生新的待确认事件；receipt 查询不受确认影响。Deliver 授权必须绑定消费方，不能让任意调用方确认别人的结果。投递语义为至少一次，消费方负责按 event_id 去重。

## 文件、容量与恢复

initialize_test 在同一私有目录的独立临时文件中完成迁移、checkpoint 和关闭，再通过不覆盖 hard link 原子发布全新数据库；中断只留下未发布的隔离文件，显式重试不会读取或接管它。已发布数据库只通过 open 恢复。废弃的 .execution-bootstrap-* 及其 sidecar 由管理员在确认没有初始化任务后清理；当前没有自动清理器。该发布要求所在文件系统支持同目录 hard link。初始化要求绝对规范路径和预建私有目录；open 只打开已有库。Unix 检查目录/数据库/WAL/SHM/journal 无组或其他用户权限、拒绝符号链接，文件创建为 0600。WAL + synchronous=FULL + foreign_keys=ON，macOS 同时启用 fullfsync/checkpoint_fullfsync；busy wait 显式有界。没有损坏重建、备用路径或隐式 authority 替换。Windows ACL 与真实 AI/用户进程隔离属于后续平台接线，不由文件 mode 测试代替。

初始 schema 与 application_id/user_version 同事务提交，无旧 prmonitor 数据库迁移或兼容层。更高版本只返回 NewerSchema 头部诊断，没有业务 Store。已打开连接的每个读/写事务也复核 schema 和 authority，不能在升级后继续使用旧写路径。坏库/不支持版本保留原文件并失败。

Limits 显式约束记录大小、批准集合、批量、消费者数、累计回执和 SQLite page ceiling。所有受保护 BLOB 读取通过同一 SQL 投影，在同一 statement 中先检查存储类型和字节长度，再物化 payload；超限/错误类型返回 Corrupt，包括打开数据库时的 authority。plan 与两类 snapshot 使用各自更紧的上限。新执行保留五个逻辑终态槽（首次取消、首次不确定、终止、首次核实、最终核实），新交互保留一个；普通操作不能吃掉它们；首次 Unknown/NotSatisfied 可落盘并进入人工核对，最终 Satisfied/NoEffect 仍有独立预留。所有幂等载体保留且随总量有界；达到长期额度需产品处理，当前没有清理、压缩或换根流程。物理磁盘满仍可能使终态落盘失败，逻辑预留不是磁盘预分配。Busy/Capacity/Storage 不触发自动业务重试。错误按可执行的恢复动作区分：

| 错误 | 调用方处置 |
| --- | --- |
| InvalidInput | 修正 operation ID、拉取参数或新聚合输入；不重配 store |
| Configuration | 修正显式 Limits 或 SQLite 持久化配置 |
| OperationCommitUnknown | 查询或重交同一 operationRequestId；绝不能换 ID 重跑 runner |
| ConfirmationCommitUnknown | 以同一 scope/consumer/event 重试 confirm；它没有 operation receipt |
| BootstrapUnpublished | 初始化 migration 提交失败，目标库未发布；诊断存储或显式重试同一路径的 initialize_test |

BootstrapUnpublished 仅来自发布前的 migration 提交；其它文件系统错误仍为 Storage。若初始化在发布后发生错误，保留目标文件并通过 open 核查，不能覆盖或重建 authority。

## 验证

~~~sh
cargo test -p execution-sqlite --locked
cargo run -p execution-sqlite --example execution-sqlite-consumer --locked -- crates/execution-contract/tests/fixtures/plan.json
~~~

真实文件测试位于 tests/execution-sqlite：重复/不同 attempt 并发、批准多记录原子回滚、无批准路径的授权版本、回答竞争、回执/确认响应丢失、乱序确认后重启补发、失败/陈旧 runner 事件的 attempt 与 directive 重放、各受保护 BLOB 超限和错误类型、三类提交失败恢复及输入/配置错误分类、锁等待、真实 SQLITE_FULL、逻辑终态容量、时钟回拨、不可变定义和不退款、重开/进程退出、迁移失败，以及文件创建/DDL/commit 后、发布前中断的公共 API 恢复、新版本诊断与旧连接禁用。macOS 专项用受限子进程实际拒绝读取/写入 DB/WAL/SHM 与目录；仅证明所配测试 sandbox，不证明生产模型进程或 Windows ACL。独立 consumer 校验实际依赖闭包，SQLite 仅在本 adapter 放行。

来源、MIT 权利依据与改写映射见[来源记录](../../docs/reference/execution-sqlite.md)。
