# execution-interaction

C04（#2397）：独立的一次性交互状态机。输入是有界引用、命令和调用方可信时间；输出是待提交的状态转换。无任务执行、授权、数据库、UI或系统时钟依赖。

`Interaction::open` → `evaluate` → `Evaluation { outcome, transition }`。Pending 的唯一终态为 Answered、Cancelled 或 Expired。时间统一 UTC Unix ms，截止排他；截止时刻回答或取消都转 Expired，绝不默认为同意。

用户确认、隐私同意与管理员授权分别建模。管理员回答只持未验证的决定引用；补参数、维护窗口、重启选择只持外部提交/选项引用。host 必须验证响应者权限、引用对应的 schema/选项/决定和 subject 绑定，不能仅凭回答类型生成批准。

相同命令ID和内容重放返回 Duplicate；相同ID不同内容/命令类型返回 IdempotencyConflict；其它终态后的命令返回 Late，不改变状态。取消仅取消本交互，窗口/AI会话关闭、drop和重开没有任务状态效果。

Expired 持久化实际触发命令（Answer、Cancel或CheckExpiry），恢复后原命令重放仍返回Duplicate，同ID不同内容仍为IdempotencyConflict。过期优先于回答种类校验，终态保存的回答不会被当作同意。Pending为所有命令变体（含其它回答种类）预留终态空间。Command采用闭合标签编码，CheckExpiry为空结构变体；当前未发布的Snapshot直接要求Expired.command，不保留缺字段旧格式读取。

Snapshot 是唯一当前格式；有界 `decode` 和 `restore` 校验版本、等待类型、时间、revision和终态。Pending 为 revision 0，唯一终态为 1。创建或恢复 Pending 时还预留最大合法终态的编码空间，避免建立无法记录回答或过期的等待。结构有效不证明存储真实，持久数据完整性由C18持有。

C18 必须在受保护主体命名空间内对 `(interaction id, expected_revision)` 做条件更新，并在冲突后重新读取、使用当前可信时间重算。T1竞争测试仅证明转换和重读语义，不宣称已实现数据库CAS。调用方负责可靠时间/回拨检测；核心拒绝早于创建或已提交事件的时间，未提交的观察不构成持久时钟水位。

验证：`cargo test -p execution-interaction --locked`；公共 API 示例为 `cargo run -p execution-interaction --example interaction-consumer --locked`。示例明确为测试状态，不运行后台任务。完整独立消费见[开发指南](../../docs/guides/contracts-development.md)。
