# AI Session Host 的状态与进程所有权

对应 [A03 #2441](https://dev.azure.com/shengming0923/rss/_workitems/edit/2441)。本方案按依赖顺序统一修改 A01 schema/ports/transition、A02 SQLite、Host/worker、A04 与客户端投影、Claude adapter，最后装配本地入口及独立产物验收。所有实现文件由主任务串行维护；探索和交付审查可独立并行。V2 wire 与 SQLite schema version 1 直接协同替换，没有数据升级、迁移或兼容读取分支。

持久事实只有一份：Session 持有绑定及可用状态，CommandRecord 持有队列和 attempt，Event 持有稳定展示，Interaction / SurfaceState 持有回调展示关联。`ProviderAgentPort.dispatch` 覆盖 prompt / steer / cancel / respond；Host 北向仍保留各语义方法。`VerifiedProviderFact` 将 dispatch、observe、reconcile 的证据绑定到 namespace、完整 provider identity 与原 attempt，Store 在提交时另做 revision / generation CAS。新接纳命令改变 revision，不使已发起的异步结果失效。

提交确认、运行状态、模型终态和控制确认分别表示不同事实。队列由 Host 提供，与 provider 原生排队能力无关。尚未派发的 queue_next 可以本地取消，取消双方记录和事件原子提交。控制命令没有模型 Outcome。unknown 不能回到 accepted，只有真实 not_submitted 证据且仍在原重试窗口内才可再次派发。

worker 启动只采用最小 durable fence：reserve → spawn → 验证 PID/PGID → register → activate。reservation 不依赖已存在 Session，因为原生绑定在 activate 后才产生。bootstrap 不提前导入 provider、读取凭据或启动 native runtime；私有 IPC 不被原生子进程继承。没有第二套恢复 socket、nonce、lease、heartbeat 或自动接管协议。

正常生命周期由当前 ChildProcess 句柄和验证过的存活 group root 授权关闭；成功同时核实 root exit 和空进程组。重启只查询已存 group 的存在性，ESRCH 才允许清除 registered fence；非空、EPERM 或未知保持阻断，不能向保存的 PID 发信号。reserved fence 不代表 SDK 曾获得激活权限，清除它也不声称旧 bootstrap 已退出。

恢复不可用由独立 Store 事务写入 `recovery_required`，保留绑定与原生坐标，已派发未决命令转 reconciliation_required，未派发旧控制输入失效，普通队列保留，旧问题与 surface 不再可操作。成功 verified rebind 才产生新 generation 并恢复 active。列表与快照包含这种状态，UI 由稳定会话事件请求 resync。

验证沿实际边界分层：契约拒绝伪造证据和错误 attempt；真实 SQLite 验证原子性及第二 owner；真实子进程验证 launch 边界、阻塞与 SIGKILL；固定模型 HTTP transport 验证真实 Claude SDK 经 Host/A04 的完整链路；tarball 在临时独立项目消费。外部模型、跨平台安装器和桌面窗口生命周期分别保留独立证据边界。
