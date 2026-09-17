# execution-lifecycle

C09 是无 I/O 的执行生命周期核心，只依赖 C01 和基础序列化/错误库。它计算状态转换、条件写入和后续建议，提供可信提交后的首次派发动作接缝，不调度重试、不自行授予产品执行权限。SQLite 原子性属于 C18，产品准入与接线属于 C19。

## 单一状态与事件

Execution 保存完整冻结计划和一个有界 Snapshot：准备状态、独立取消标记、首尝试时间、累计预算、当前 attempt、最新事件。phase/directive 由事实推导，不保存另一份成功/重试状态。历史 attempt 与事件唯一性由 C18 journal 持有。

open 建立 Received；evaluate(Event, now, verifier) 产生含 expected_revision 的 Transition。只有条件写入提交成功后候选才是 authority。最新完全相同事件返回 Duplicate；相同 ID 不同内容拒绝；旧 revision 返回 Stale，未来/溢出 revision 拒绝。C18 须保证所有历史事件和 attempt ID 唯一，不能用新 revision 重新提交历史命令。

Transition 为私有构造且不可复制，以 next()/expected_revision() 提供候选读取。commit 消费候选并调用可信持久化回调：首次原子接纳返回 Applied；重放返回 AlreadyCommitted；CAS 冲突、失败和结果不明返回错误。只有 BeginAttempt 的首次成功提交产生不可复制、不可反序列化的 DispatchAction；其 dispatch 消费 self，绑定精确计划、attempt、runner、mode 与提交 revision。其它状态转换不产生动作。C18/C19 回调必须兑现授权重检和原子写入，类型本身不证明外部写入成功。

首次动作只活在内存中：提交后崩溃或丢弃动作，恢复的 Starting 一律 Reconcile；不能从 Snapshot 再造首次动作。宿主派发前仍检查停止/取消与 runner 能力，派发结果不明进入核对，不重试旧动作。

decode/restore 只接受当前格式 version=1，拒绝未知字段/版本、非法绑定、时间/预算/状态组合和超大输入，不提供旧版兼容或失败回退。恢复输入必须来自经过认证的受保护 journal；可反序列化的 Snapshot 本身不是执行许可。显式 Limits 至少保留 16 KiB 快照空间，当前有限字段和 C01 ID 上限使终止/核实记录可在该空间内落地。

## 准入、取消、退出与核实

BeginAttempt 表示执行 intent 接纳候选，阶段为 Starting，不声称进程已经启动。必须由 C19 先重新授权，再由 C18 与批准消费一起原子接纳。首 attempt 接纳启动总时钟；后续等待/重试不重置时钟，已接纳失败尝试也计数。输出按每次 attempt 累计，再汇总全部 attempt，包括丢弃字节；活动期减少计数拒绝，溢出只会耗尽预算。
Exited/NeverDispatched 必须携带可信最终输出总量，与终止事实同时结算；最终量不得小于已上报量，终止后不可增加。迟到的部分计数不会减少已结算值；旧 attempt 替换前已全额入账，不能因输出消息乱序恢复预算。

Cancel 只保留停止意图；Recover 将未终止 attempt 标记 Unknown 并保留曾派发的单调事实，不能再接受冲突的 NeverDispatched。StopRunner 是建议，不能冒充退出或回滚。预算/有效期耗尽阻止新尝试并建议停止，仍接受迟到的退出、已结算范围内的输出与核实证据。
StopReason/LimitReason 区分取消、尚未生效、过期、输出、总时长与尝试次数；同时发生时按取消、有效期、输出、超时、尝试次数顺序给出原因。宿主将原因与裁决时间写入审计，不将超时建议记成进程退出。ObservationVerification 保留 Unavailable/Untrusted 分类，成功返回后的绑定失败另报 Observation；均不携带外部文本。

Observe 只接收 EvidenceRef，必须经 ObservationVerifier 认证来源、完整计划/attempt/runner 绑定、证据类别和观察时间。Exited 必须证明本次受控执行及其委派工作均停止；仅父 shell 退出而子进程继续活动应返回 Uncertain。NeverDispatched 必须有权威派发记录，不能从“未发现进程”推断。退出码 0 仍须独立核实目标，不直接成为成功。

只有停止且核实 NoEffect，或权威证据证明 NeverDispatched，才可 RetryEligible；NotSatisfied 可能已有副作用，Unknown 不证明无副作用，均进入 ManualReview。还必须满足未取消、总时长/输出与尝试数预算。测试 mode 只能消费 TestResult；真实 mode 区分 ProcessExited/StateObserved；Test authority 禁止 Real，重试不可更换 mode/runner。Done 也可能代表取消后的无副作用结束，读取方必须同时展示 mode 与 assessment。

## 验证与外部职责

~~~sh
cargo test -p execution-lifecycle --locked
cargo run -p execution-lifecycle --example lifecycle-consumer -- crates/execution-contract/tests/fixtures/plan.json
~~~

测试覆盖取消/恢复、退出码与核实分离、无副作用重试、累计预算、幂等/CAS、错 attempt/证据/时钟、严格恢复与终态空间。独立 consumer 使用显式测试证据，不产生 OS 副作用。

补充矩阵覆盖 Real 模式下 ProcessExited/StateObserved/TestResult 的 live 与已记录证据 restore 交叉拒绝；它只证明核心类别规则，不证明真实 OS 来源。终态最终输出 99 小于已记账 100 时拒绝且快照/revision 不变。Cargo manifest 为全部 targets 启用认知复杂度 lint。

真实时钟回拨/跨重启时长、进程树停止、证据真实性、受保护存储、原子事务和平台执行须由后续 adapter 提供 T2/T3 证据；本核心测试不能代替。来源见[执行核心来源](../../docs/reference/execution-cores.md)。
