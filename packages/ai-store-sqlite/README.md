# AI SessionStore SQLite

持有产品会话、命令账本、稳定展示与跨服务交付，复用公共契约的纯转换。原生 transcript 与 Rust 执行库分别由各自 owner 持有。API、配置和运行版本约束见 [src](src/) 与 [manifest](package.json)。

## 所有权与恢复

一个 Host 独占整个数据库。唯一长连接在访问 WAL 前设置 `locking_mode=EXCLUSIVE`，完成真实写事务后才返回成功；事务结束仍保留所有权。第二个进程、诊断连接或其他 Host 的打开请求会有界失败。关闭或进程死亡释放锁，随后新 Host 可打开。数据库必须放在本机文件系统，不能把网络共享文件当作多 Host 协调器。

打开既有数据库只恢复读取能力。旧会话的新接纳、commit、retire 必须先通过 `VerifiedProviderSession.restore` 与 `store.rebind` 建立新 generation。既有同内容 receipt 的读取不授予重发权限。rebind 同事务保存新 binding/capabilities、原 attempt 的 observer、旧回调/surface 失效和稳定事件；原始 generation、native session/run/request、unknown correlation 不可改写。没有 lease、claim、后台 worker 或第二份 provider 派发队列。

## 数据与故障

数据库使用本机私有目录，不能放在网络共享上协调多个 Host。POSIX 拒绝过宽权限与文件别名；Windows ACL 由平台组合根落实。SQLite/WAL 包含敏感会话文本且并非整体加密，备份和账户隔离由产品负责。

命令、事件、交互、surface、delivery 与版本更新同事务提交，失败全部回滚。恢复 UI 不复活原生回调，未知副作用先核实。只有无未决工作且保留期满足时才清理，不能为了腾容量删除活跃幂等记录。

既有库先只读验证身份与格式，不支持或损坏时保留文件并拒绝打开；不自动迁移、删除或重建。新建失败留下的文件需操作方核查，不把它当成新库覆盖。容量限制不是 WAL 总量的硬配额，磁盘满仍可能使提交失败。

关闭先停止新操作，只有实际 SQLite 关闭成功才释放所有权。取消或失败后用新预算重试；同步原生 IO 不能被 JS timer 抢占，超时不证明锁已释放。损坏与未知 IO 不盲目重放。

WAL 依 SQLite 文件系统同步契约请求耐久性；进程崩溃测试不能证明断电、控制器故障或任意网络文件系统安全。来源与许可见[来源记录](../../docs/reference/ai-store-sqlite.md)。
