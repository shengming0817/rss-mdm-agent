# AI Session Host 的状态与进程所有权

对应 [A03 #2441](https://dev.azure.com/shengming0923/rss/_workitems/edit/2441)。本方案按依赖顺序统一修改 A01 schema/ports/transition、A02 SQLite、Host/worker、A04 与客户端投影、Claude adapter，最后装配本地入口及独立产物验收。所有实现文件由主任务串行维护；探索和交付审查可独立并行。AGENT-AI-01 当前为 V5 wire 与 SQLite schema version 4，直接协同替换旧版本，没有数据升级、迁移或兼容读取分支。

持久事实只有一份：Session 持有绑定及可用状态，CommandRecord 持有队列和 attempt，Event 持有稳定展示，Interaction / SurfaceState 持有回调展示关联。`ProviderAgentPort.dispatch` 覆盖 prompt / steer / cancel / respond；Host 北向仍保留各语义方法。`VerifiedProviderFact` 将 dispatch、observe、reconcile 的证据绑定到 namespace、完整 provider identity 与原 attempt，Store 在提交时另做 revision / generation CAS。新接纳命令改变 revision，不使已发起的异步结果失效。

提交确认、运行状态、模型终态和控制确认分别表示不同事实。队列由 Host 提供，与 provider 原生排队能力无关。尚未派发的 queue_next 可以本地取消，取消双方记录和事件原子提交。控制命令没有模型 Outcome。unknown 不能回到 accepted，只有真实 not_submitted 证据且仍在原重试窗口内才可再次派发。

Host 独立 `WorkerLaunchFenceStore` 持有进程 fence 类型与校验，SQLite 同时实现该 port 和公共 `SessionStore`，组合根分别注入。A01 不拥有 artifact/PID/PGID 或 OS 测试替身。worker 启动只采用最小 durable fence：reserve → spawn → 验证 PID/PGID → register → activate。reservation 不依赖已存在 Session，因为原生绑定在 activate 后才产生。bootstrap 不提前导入 provider、读取凭据或启动 native runtime；私有 IPC 不被原生子进程继承。没有第二套恢复 socket、nonce、lease、heartbeat 或自动接管协议。

正常生命周期由当前 ChildProcess 句柄和验证过的存活 group root 授权关闭；成功同时核实 root exit 和空进程组。重启只查询已存 group 的存在性，ESRCH 才允许清除 registered fence；非空、EPERM 或未知保持阻断，不能向保存的 PID 发信号。reserved fence 不代表 SDK 曾获得激活权限，清除它也不声称旧 bootstrap 已退出。

恢复不可用由独立 Store 事务写入 `recovery_required`，保留绑定与原生坐标，已派发未决命令转 reconciliation_required，未派发旧控制输入失效，普通队列保留，旧问题与 surface 不再可操作。成功 verified rebind 才产生新 generation 并恢复 active。列表与快照包含这种状态，客户端独立投影 `SessionView.sessionStatus`，稳定会话事件触发 resync；attached 不覆盖持久的 recovery_required。

验证沿实际边界分层：契约拒绝伪造证据和错误 attempt；真实 SQLite 验证原子性及第二 owner；真实子进程验证 launch 边界、阻塞与 SIGKILL；固定模型 HTTP transport 验证真实 Claude SDK 经 Host/A04 的完整链路；tarball 在临时独立项目消费。外部模型、跨平台安装器和桌面窗口生命周期分别保留独立证据边界。

关闭所有异步等待共用绝对 deadline；超时同步断开 IPC/工具桥并升级当前持有 worker 的终止，失败保留可重试状态。恢复写库失败也必须立即隔离 runtime 并输出闭合 diagnostic。Claude transcript 目录与凭据同属本地私有文件系统边界。运行包 Node 版本从根 manifest 读取，archive checksum 与 SQLite 版本按 `(version, platform)` 映射，未知组合在下载前失败；app 组合根纳入依赖和 builtin/import 边界检查。

## 个人连接与 provider 阶段

AGENT-AI-01 将产品会话生命周期与 native context 分开。`Session.stages` 从同一 JSON Schema 生成，空会话无 binding；`currentStageId` 指向当前阶段，`selectedConnectionId` 是下一次普通输入所用连接。`freshContext` 仅表达下一条输入的新上下文意图。Receipt 固定接纳 stageId；每个阶段保留 connectionId、configRevision、credentialRevision、binding 与 capabilities。原阶段历史不被切换覆盖。

Host 先检查原 commandId 回执，后核对已确认历史，再等待旧队列全部结算并打开新阶段。连接修订与偏好按 Caller 的 tenant/principal/authority 存储，revision 只追加。原生 caller 由本地 ingress 根据用户 registry 与 generation 注入。一个 Host 承接所有用户；用户切换取消模型工作及验证 worker，Rust 设备任务继续持有冻结原 actor，不重绑定执行 app。

配置快照仅含非秘密连接声明，credentialRef 由 native broker 解析；临时验证 namespace 不生成产品会话。模型探针完成和 worker 停止后才保存新连接。登录观察身份按用户/连接修订固定，API secret 不参与身份哈希。恢复读取原阶段修订与 native context 索引；普通 token 刷新仅允许同来源、同账号。历史预览是普通新输入的一部分，不建立独立切换任务或隐式重发链。

`test-users` 与旧 `s1` 数据根隔离，无默认 actor、自动数据归属或兼容配置分支。相关操作及来源证据见 [Host 应用](../../apps/ai-host/README.md)。
