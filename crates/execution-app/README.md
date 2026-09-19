# 无 UI 受控执行应用服务

C19 的 Rust 组合根，连接 C06 能力、C07 唯一授权裁决、C08 批准、C09 生命周期与 C18 SQLite。不依赖 UI、MCP、AI SDK 或 AI 会话存储；C20 负责把 UI/AI 公共操作接到这些 API。

## 公共路径

- `start(CreateTest/OpenTest)` 显式使用 Test authority；缺可信绑定立即失败。更高 schema 返回 `NewerSchema { found, supported }`，不产生可执行/确认的应用句柄，保留原库。`Production` 在 S1 一律拒绝，不自动创建数据库或降级为测试身份。
- `submit(request, frozen_plan)` 保持原 request/plan/digest。相同业务请求返回现存状态，不重新准入、消费审批或派发；不同内容冲突。内部日志 ID 由 authority/request/阶段/命令确定性派生，没有第二套请求映射表。
- `advance(request, command)` 是服务 owner 的显式新尝试，不是提交重放。能力预检 → 可信快照 CAS → 事务内当前能力/C07/C08 → 批准消费、意图、审计、回执原子提交 → 当前取消/预算/能力检查 → 消费一次性派发值。
- `reconcile` 只获取可信 runner 的终止和独立效果事实，不产生新尝试。退出零不等于已核实；丢失 runner 记录保持 Unknown，不根据计划合成结果。首次派发值不能序列化、复制或从数据库恢复。
- 未确认派发以 `DispatchUnconfirmed` 保存精确 attempt 与闭集原因：能力、时钟、配置、权限、取消、具体预算/有效期、陈旧 revision、runner 不匹配、生命周期变化、runner 拒绝/错误或投递未知。C18 审计在陈旧或拒绝事件上也保留它们；当前 attempt 的原因同时进入安全状态投影。原因不替代终止/效果证据，不退款或触发重派。时钟/存储本身无法支持事务时明确返回错误，不伪造可靠时间或已持久化诊断。
- `cancel` 仅用 Execute 持久化取消请求；独立服务 owner 的 `reconcile` 统一请求 stop 并以 RunnerFact 记录 `StopReported` 的 Acknowledged/Failed，不等于已终止或已回滚。stop 失败后仍观察终止和效果；诊断写入失败也不跳过观察，并向调用者报告该错误。降级状态仍可读、取消和核对。
- 交互 `open_interaction/interaction/respond` 使用 C04/C18，答案和管理员记录引用均不是执行批准；显式新尝试仍由可信 host 提供 C08 事实。交互回答不自动续跑任务。
- `pull_results/confirm` 逐事件可靠投递，消费者先持久处理再确认；查询/确认不派发。`audit` 另需当前 ReadAudit 权限。

任务定位携带封闭的 `ExecutionAccess`：执行/取消使用 Execute，核对使用 RunnerFact，交互写入使用 Interact，投递/确认携带精确 consumer，审计使用 ReadAudit；不会附加 ReadResult。普通 `status/interaction` 仍需 ReadResult。内部 RunnerFact/ManageTrust 是可信服务权限，不由调用 DTO 授予。已提交操作的安全回执允许原动作权限或独立 ReadResult 权限读取；只读重放不重新申请执行批准。

普通命令通过 C18 `apply_command` 提交；只有观察路径创建携带当前可信 runner 事实的验证器，调用 `apply_observation`。普通 Host 不持有可选观察槽，观察提交保留当前 revision 对操作身份的绑定。

准入拒绝是持久业务结果：C18 在原回执内生成 `AdmissionStatus`，应用在同一读取事务中恢复生命周期与最近准入投影。首次、重放和重启均返回 Denied/ApprovalRequired，而非瞬时错误后变回 Waiting；批准人、规则与完整裁决仍只在特权审计中。已有 attempt 的事实和新的准入结果分别呈现。

`ExecutionApp` 由服务生命周期持有，调用均同步、有界；UI 窗口或模型调用只拥有请求/响应，不能拥有执行 future。owner 独立调度 `reconcile`，新尝试则使用显式稳定 command ID。S1 没有常驻 OS 服务安装器、通用 worker 框架或任意 exec/PTY 接口。

`RequestId` 只标识任务，`CommandId` 标识任务内一次逻辑调用，两者不能混用。适配器须持久保留命令 ID，网络重试不能生成新值：

| 情况 | 恢复动作 |
| --- | --- |
| `submit` 响应丢失或提交未知 | 同 request/冻结内容重放或 `status`；不得生成替代 request |
| 已注册、尚无 attempt，首次执行发生暂时失败 | owner 用 `advance(request, CommandId::initial_attempt())` 继续原首次命令；已持久拒绝仍返回安全准入投影 |
| `advance` 失败/未知 | 重试原 CommandId；只有明确请求新的授权尝试才创建新 CommandId |
| `respond` 失败/未知 | 原 CommandId、交互 ID 和同一答案重放；不同内容复用键会冲突 |
| `ConfirmationUnknown` | 用原 request/consumer/event 再次 `confirm`，不要查询不存在的业务操作回执 |

取消和核对的内部事实写入以当前 revision 区分 CAS 重算；陈旧写入不会永久吞掉取消或证据。核对只在读取持久标记或预算 stop directive 后请求停止；取消持续竞争则有界返回 Conflict 供调用方重试。owner 必须独立调度核对，取消响应本身不保证停止已经开始。

## 可信边界和限制

`AppHost` 必须独立认证 authority/actor/device、来源账号/委托、规则、能力、批准与可靠时钟；在 SQLite 事务内的回调必须有界、非重入并消费已验证的本地快照，不能请求网络。提交字段、UI/AI 声明或 provider 登录不能成为授权事实。host/runner 是可信进程内代码，不能抵抗恶意实现或本机管理员。

`DeterministicTestRunner` 仅有有界内存记录：完成、等待、取消、拒绝派发、未知和无效果。既不执行计划的 launch，也不访问目标文件/网络/系统进程。全部证据为 `TestResult`，投影始终带 Test 模式。全新 runner 无法恢复旧内存事实，必须保持 Unknown；真实 runner 的持久核对属于后续平台任务。

配置接缝由 `Configuration` 持有一个当前版本：硬上限验证、可信管理员审计成功后原子替换、LKG 或禁止新执行的 degraded。可信 host 持有配置加载/持久化和审计 sink；强制版本下限不是客户端输入。存储 bootstrap 限额固定且不热替换。重启时 host 必须再次提供符合当前强制政策的配置；S1 不新增配置表、生产配置文件存储或兼容读取器。

C18 继续独占 migration。request 恢复按精确动作分权，回执保存安全准入投影；C09 持有 attempt 的派发/stop 诊断及终止/效果事实。不新增表或应用层数据库，不保留旧接口别名、旧 GateRejected 分支或通用 condition 框架。

## 验证

```sh
cargo test -p execution-app -p execution-sqlite -p execution-lifecycle --locked
cargo run -p execution-app --example execution-app-consumer --locked -- crates/execution-contract/tests/fixtures/plan.json
```

验收使用真实私有临时 SQLite 文件、真实线程竞争与崩溃子进程，覆盖审批/提交回滚、丢响应重放、取消/降级、策略变化重试、交互不能授权、审计隔离和乱序确认。子进程仅运行测试二进制并在提交点退出，不执行计划。无真实设备、软件安装、AI 引擎或 OS 隔离验收声明。

`scripts/check-rust-consumers.mjs` 在隔离 workspace/target/features 中复制独立示例并核对 metadata 闭包；示例证明响应通道断开后服务仍完成、重新打开数据库查询且不重发。正式证据由提交源码上的完整 `make ci CI_BASE=origin/develop` 记录在被忽略的 `.local-ci-runs/`。

## 来源

直接读取 kube-rs 固定 revision [`f3619c349faebb4af25df013498af5f2bb85d1f5` 的 `kube-runtime/src/controller/mod.rs`](https://github.com/kube-rs/kube/blob/f3619c349faebb4af25df013498af5f2bb85d1f5/kube-runtime/src/controller/mod.rs)，借鉴显式调谐建议与实际调度分离；本实现无上游源码复制或依赖，不引入 Kubernetes 资源模型、隐式自动重试或 worker。一次性权限与 SQLite 原子性直接消费仓内 C09/C18，无第二份机制。目标新源码沿用本仓 MIT。

本轮分权与诊断设计另读取 [Kubernetes v1.34.0 authorizer/interfaces.go](https://github.com/kubernetes/kubernetes/blob/v1.34.0/staging/src/k8s.io/apiserver/pkg/authorization/authorizer/interfaces.go) 的动作/资源属性、[apimachinery v0.34.0 types.go](https://github.com/kubernetes/apimachinery/blob/v0.34.0/pkg/apis/meta/v1/types.go) 的状态/原因分离，以及 [Axum 0.8.4 extract/state.rs](https://github.com/tokio-rs/axum/blob/axum-v0.8.4/axum/src/extract/state.rs) 的集中上下文与窄状态提取。仅借鉴模式，不复制代码、不新增依赖；Host 私有字段只经一个构造入口初始化。

## 授权任务详情

`task_details(request_id)` 以 ReadResult 一次读取 ExecutionRecord，并再次核对当前 authority/actor/device binding，返回同一记录的 `ExecutionStatus` 和 `FrozenPlanSummary`。`status` 与它共享投影；不需要 ReadAudit，也不暴露 parameters、argv、cwd、env、stdin、路径/网络明细或批准记录。摘要包括原 plan ID/digest、目标、运行身份、精确资源/制品/解释器、策略版本、有效期、预算与访问数量。`scripts/check-execution-bindings.mjs` 从 Rust 的序列化 schema 生成桌面类型，同时用真实 SQLite + DeterministicTestRunner 生成 running、approvalRequired、outcomeUnknown、testCompleted、cancelled 五种 fixture；没有手写并行执行 DTO。
