# AI Session Host 的状态与进程所有权

Host 持有产品会话和可靠交付，原生引擎持有模型上下文，Rust 服务持有设备执行权威。

持久事实只有一份：Session 持有绑定及可用状态，CommandRecord 持有队列和 attempt，Event 持有稳定展示，Interaction / SurfaceState 持有回调展示关联。`ProviderAgentPort.dispatch` 覆盖 prompt / steer / cancel / respond；Host 北向仍保留各语义方法。`VerifiedProviderFact` 将 dispatch、observe、reconcile 的证据绑定到 namespace、完整 provider identity 与原 attempt，Store 在提交时另做 revision / generation CAS。新接纳命令改变 revision，不使已发起的异步结果失效。

提交确认、运行状态、模型终态和控制确认分别表示不同事实。队列由 Host 提供，与 provider 原生排队能力无关。尚未派发的 queue_next 可以本地取消，取消双方记录和事件原子提交。控制命令没有模型 Outcome。unknown 不能回到 accepted，只有真实 not_submitted 证据且仍在原重试窗口内才可再次派发。

Host 独立 `WorkerLaunchFenceStore` 持有进程 fence 类型与校验，SQLite 同时实现该 port 和公共 `SessionStore`，组合根分别注入。A01 不拥有 artifact/runtimeDigest/平台 scope 或 OS 测试替身。worker 启动只采用最小 durable fence：reserve → spawn → 验证当前 launcher/worker 与平台 scope → register → activate。reservation 不依赖已存在 Session，因为原生绑定在 activate 后才产生。bootstrap 不提前导入 provider、读取凭据或启动 native runtime；私有 IPC 不被原生子进程继承。没有第二套恢复 socket、nonce、lease、heartbeat 或自动接管协议。

正常生命周期由当前 ChildProcess 句柄和验证过的存活 group root 授权关闭；成功同时核实 root exit 和空进程组。重启只查询已存 group 的存在性，ESRCH 才允许清除 registered fence；非空、EPERM 或未知保持阻断，不能向保存的 PID 发信号。reserved fence 不代表 SDK 曾获得激活权限，清除它也不声称旧 bootstrap 已退出。

恢复不可用由独立 Store 事务写入 `recovery_required`，保留绑定与原生坐标，已派发未决命令转 reconciliation_required，未派发旧控制输入失效，普通队列保留，旧问题与 surface 不再可操作。成功 verified rebind 才产生新 generation 并恢复 active。列表与快照包含这种状态，客户端独立投影 `SessionView.sessionStatus`，稳定会话事件触发 resync；attached 不覆盖持久的 recovery_required。

验证沿实际边界分层：契约拒绝伪造证据和错误 attempt；真实 SQLite 验证原子性及第二 owner；真实子进程验证 launch 边界、阻塞与 SIGKILL；固定模型 HTTP transport 验证真实 Claude SDK 经 Host/A04 的完整链路。外部模型、跨平台安装器和桌面窗口生命周期分别保留独立证据边界。

关闭所有异步等待共用绝对 deadline；超时同步断开 IPC/工具桥并升级当前持有 worker 的终止，失败保留可重试状态。恢复写库失败也必须立即隔离 runtime 并输出闭合 diagnostic。Claude transcript 目录与凭据同属本地私有文件系统边界。运行包 Node 版本从根 manifest 读取，archive checksum 与 SQLite 版本按 `(version, platform)` 映射，未知组合在下载前失败；app 组合根纳入依赖和 builtin/import 边界检查。

## 个人连接与 provider 阶段

产品会话与原生上下文分开，首条输入才固定 provider 阶段。连接切换不覆盖原阶段历史，接纳回执保持原归属。具体字段以 schema 为准。

Host 先检查原 commandId 回执，后核对已确认历史，再等待旧队列全部结算并打开新阶段。连接修订与偏好按 Caller 的 tenant/principal/authority 存储，revision 只追加。自定义 API 密钥的加密 AAD 绑定 provider、规范化 endpoint 与 credential type；任一目标变化都要求重新输入。HTTPS 字面私网地址在解析前拒绝，worker 激活还要求 DNS 的全部地址均为公网。原生 caller 由本地 ingress 根据用户 registry 与 generation 注入。一个 Host 承接所有用户；用户切换取消模型工作及验证 worker，Rust 设备任务继续持有冻结原 actor，不重绑定执行 app。

provider activation 通过既有 worker 私有管道传递，不写快照或来源账号文件。配置声明与内部密文由同一 SQLite 事务持有；Host 组合根解密，只把当次所需秘密交给 worker。主密钥由 Native 延迟提供，worker 不持有主密钥。已有配置直接交由官方 CLI/SDK 解析及认证，RSS 不处理外部 token、账户身份或刷新。临时验证 namespace 不生成产品 Session，探针完成且进程停止后才保存；编辑保留或替换密钥，删除清除全部密文并阻止新 worker。恢复只读取 RSS 自有 native context 索引。历史预览仍是普通新输入的一部分。

Native–Host 使用私有继承 stdin/stdout 的有界承载，native/execution 两条逻辑通道共用同一 owner；Native 用户注册表产生可信 Caller/generation，UI 只能在绑定逻辑通道内通信。Host 只从当前 Native 上下文生成 execution-origin，Rust MCP 逐调用核对 principal 与 generation，协议 metadata 不能自证身份。设备执行服务按 authority/device 绑定，用户操作显式携带 RequestContext，内部核对以任务冻结 actor 授权。仅一个队列、SQLite owner 和执行线程，无按用户服务池；无任务时阻塞等待。

当前没有历史数据兼容或迁移。[#2462](https://dev.azure.com/shengming0923/rss/_workitems/edit/2462) 增加独立安全状态服务的一次性 challenge，不向 AI worker 分发服务凭据；HMAC、凭据票据与 worker grant 不采用。进程承载、恢复与实验室边界见[安全服务架构](local-service.md)。AES-GCM 随机 IV 保留。运行和验证边界见 [Host 应用](../../apps/ai-host/README.md)。
