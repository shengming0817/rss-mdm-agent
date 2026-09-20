# AI SessionStore SQLite

`@rss-mdm-agent/ai-store-sqlite` 实现 A02 / #2440 的 Node/TypeScript `SessionStore`。依赖 A01 公共契约的同步状态转换，持有产品会话、命令账本、稳定事件、交互、surface 关联及跨服务 delivery。原生模型 transcript、Rust 批准/执行 intent/结果表由各自 owner 持有。

## 打开与所有权

```ts
import { openSqliteStore } from "@rss-mdm-agent/ai-store-sqlite";

const opened = openSqliteStore({
  path: "/private-runtime/ai/session.sqlite",
  mode: "create", // 已有数据库必须显式使用 open
});
if (!opened.ok) throw new Error(opened.error.code);
const store = opened.value;
// 组合根通过 A01 的可信 provider 准入构造 Session，随后调用 store.create。
// store.accept / commit 成功返回后才能唤醒 mailbox 或发布稳定事件。
await store.close({ timeoutMs: 1000, signal: new AbortController().signal });
```

验证并限定 Node **24.14.1** 内置 `node:sqlite` / SQLite **3.51.2**；其他 runtime 明确拒绝，不自动切换 driver。该 Node API 仍带 experimental 标记。包不依赖 native addon、模型 SDK、UI 或 Rust sidecar；peer `ai-contract` 与 `ai-host` 固定0.1.0；仅消费后者的 `launch-fence` 入口，组合根安装同一份契约及生命周期 artifact。

一个 Host 独占整个数据库。唯一长连接在访问 WAL 前设置 `locking_mode=EXCLUSIVE`，完成真实写事务后才返回成功；事务结束仍保留所有权。第二个进程、诊断连接或其他 Host 的打开请求会有界失败。关闭或进程死亡释放锁，随后新 Host 可打开。数据库必须放在本机文件系统，不能把网络共享文件当作多 Host 协调器。

打开既有数据库只恢复读取能力。旧会话的新接纳、commit、retire 必须先通过 `VerifiedProviderSession.restore` 与 `store.rebind` 建立新 generation。既有同内容 receipt 的读取不授予重发权限。rebind 同事务保存新 binding/capabilities、原 attempt 的 observer、旧回调/surface 失效和稳定事件；原始 generation、native session/run/request、unknown correlation 不可改写。没有 lease、claim、后台 worker 或第二份 provider 派发队列。

## 事务与读取

- `BEGIN IMMEDIATE` 内只执行同步状态转换和 SQL；没有可注入 async 事务回调。命令、事件、交互、surface、delivery 先写，带 revision/generation 的 session CAS 最后写；任何失败全部回滚。
- 每张主键、唯一键、外键及作用域查询都包含 tenant/principal/authority/session。session 内 event ID 和 sequence 唯一；callback ID 在 namespace/generation 内唯一；delivery 引用同 namespace 的原事件。
- surface 的原样上游 messages 随完整 SurfaceState 同时保存在状态和稳定事件中，身份/revision/lifecycle 与恢复内容原子推进。历史 payload 与问题可重放；恢复 UI 不复活原生 callback。
- snapshotPage 在当前同步连接捕获一致性读视图，按同一 cursor 分页全部稳定历史；listSessions 按可信 caller 隔离并分页。两者续页缓存最多128份/16MiB、30秒期限，关闭/重启后返回 cursor_expired；重新抓取快照即可按持久水位接续。不压缩活动日志，不静默截断。events 从 exclusive cursor 接续。全局 recovery/deliveries 使用有界 keyset 分页，cursor 仅向相同 adapter 原样回传。
- delivery 查询同时返回到期 pending 与 reconciliation_required。调用方检查 status/retry，未知副作用先核实；查询不是领取或自动重发许可。重复 delivered 结算保持幂等，不承诺跨数据库/外部副作用 exactly-once。
- 全部命令 terminal/invalidated、无 pending interaction 且 delivery 全部 delivered 才能 retire。receipt 保留期结束后可 prune；永久 namespace tombstone 防止 session ID 复用。active 命令、历史去重键和未决副作用不会为了腾容量而删除。

## Schema、安全与容量

独立 application ID 与 schema version 3（AI wire V4）。`open` 先通过临时只读连接验证身份与 schema，不改变外部数据库；取得长期写连接后，在写事务内重新验证，再设置持久配置。首次建库的 DDL、版本和 checksum 在同一事务；已有 schema v1/v2、缺版本、缺表、对象变化、较新版本或校验失败时拒绝打开；旧库只读检查，不自动修改、删除或重建。没有旧 PR 数据库导入、双读、fallback 或损坏后重建。首次建库失败可能留下无 schema 的文件；必须由操作方检查并处置，`open` 不将其当成新库。

`path` 必须是本机私有目录中的绝对文件路径。所有数值必须为安全整数；组合根通过 `StoreOptions` 选择下列范围内的值，超过边界返回 `invalid_input`。运行中超过容量预算返回 `limit_exceeded`。

| 选项 | 默认值 | 有效范围（含边界） | 约束对象 |
| --- | --- | --- | --- |
| busyTimeoutMs | 1000 | 1–10000 | SQLite 锁等待毫秒 |
| maxDatabaseBytes | 256 MiB | 65536–1073741824 | 主数据库页数上限；WAL 保留目标同值 |
| maxSessionRecords | 10000 | 1–100000 | 会话及其事件、投影和 generation 历史 |
| maxSessionBytes | 16 MiB | 1–1073741824 | 会话持久记录 JSON 与 generation 字节总量 |
| maxBatchRecords | 1024 | 1–100000 | 一次 commit 的输入记录与 prune 的会话数 |
| maxQueryBytes | 4 MiB | 1–1073741824 | 每次查询返回总量；分页先在 SQL 中核对原始字节量 |

单条记录继续使用 A01 的262144字节及字符串/节点/深度预算；页大小最多1024。主文件通过 `max_page_count` 限制，WAL 自动 checkpoint 为256页；WAL 在事务/checkpoint 期间需要额外磁盘空间，`maxDatabaseBytes` 不是主文件与 WAL 总和的硬配额。磁盘满返回 limit_exceeded，锁/存储错误返回不含原始 SQL、路径、正文或 provider 数据的 Failure。

POSIX 新目录0700、文件0600；已有目录/文件过宽、文件为 symlink 或 hard link 均拒绝。Windows 使用宿主用户目录 ACL，组合根负责专用账户与目录准入；本次 POSIX 权限测试不替代 Windows ACL 验收。扩展加载关闭、双引号字符串关闭、foreign_keys 开启、trusted_schema 关闭。配置只存引用，session 文本本身仍是敏感数据；SQLite 文件/WAL 未加密，密钥托管、备份和账户隔离归产品装配。

## 验证与耐久边界

```sh
pnpm test:ai-store
pnpm check:ai-store-consumer
make ci CI_BASE=origin/develop
```

测试使用真实 SQLite 与独立子进程：同键重复/异内容、最后一步 SQL/FK 失败回滚、SQLITE_FULL、schema 中断/较新拒绝、surface snapshot 接续、receipt 丢失、commit 后 publish 前 SIGKILL、intent/unknown 恢复、独占与所有权交接。共用 A01 conformance 同时覆盖两种存储。独立 tarball consumer 在仓外构建并执行公共 API、恢复与双进程所有权，记录源码 SHA、lock/tarball 摘要、平台/runtime 与结果；脏源码不产生通过的交付记录。

WAL 使用 `synchronous=FULL`，按 SQLite 的文件系统同步契约请求提交耐久性。SIGKILL 测试证明进程崩溃恢复；没有执行真实断电、存储控制器故障、网络文件系统或真实模型/OS 工具隔离验收，不能以进程测试替代这些证据。

固定源码对标、来源取舍与许可见[来源记录](https://dev.azure.com/shengming0923/rss/_git/rss-mdm-agent?path=%2Fdocs%2Freference%2Fai-store-sqlite.md&version=GCeb7aa3c52bd91bd764eead511d58a11b78fea716)。

## 关闭与错误边界

仓库与包的 engines 都精确声明 Node 24.14.1。`make ci` 在执行检查前读取根 package.json 并拒绝其他 Node 版本；包打开时读取自身 manifest 检查同一版本，独立安装不依赖仓库文件。

`close(budget)` 首先停止新操作，再校验预算与预先触发的 AbortSignal。取消或原生关闭失败不会被报告为成功；连接可能仍持锁，调用方必须用新预算重试 close。只有实际 DatabaseSync.close 返回成功才进入 closed 并释放内存 ownership；随后重复关闭幂等。同步 SQLite 原生 I/O 不能被 JavaScript timer 或 AbortSignal 抢占，timeoutMs 不构成原生关闭的硬截止保证；没有虚构 Worker 隔离或超时即已释放锁的承诺。验证覆盖预取消仍持锁、停止读写、关闭失败与新预算重试后的再次打开。

SQLite 扩展结果码按低8位取得稳定 primary code，再投影到产品 Failure；不保留 SQL、文件路径或原生异常文本。

| 原因 | code | retry |
| --- | --- | --- |
| BUSY / LOCKED（含扩展码） | unavailable | same_command |
| CORRUPT / NOTADB、已确认本产品 schema 的对象/元数据/完整性损坏、持久 wire 非法 | storage_corrupt | never |
| FULL | limit_exceeded | never |
| PERM / READONLY / AUTH、EACCES / EPERM | permission_denied | never |
| 路径缺失、create 已存在、错误路径种类 | invalid_input | never |
| 外部 application ID、未知 schema 版本、未验证 runtime | unsupported_version | never |
| 未知错误与其它 I/O 故障 | unavailable | never |

损坏或未知 I/O 不触发盲目重放、自动删除或重建。双进程测试在 contender 内计量实际 open 耗时，对 busyTimeoutMs=50ms 要求 <1000ms 宽松上界，并验证明确的 unavailable/same_command；进程启动耗时不混入锁等待测量。重启后的 reconciliation 使用 A01 的 VerifiedProviderSession 实际调用凭证，不能再手写 not_submitted 结构授权重新派发。Id 验证直接使用 A01 从唯一 schema 编译的 isId，无复制正则。


A03 的 `WorkerLaunchFenceStore` 独立于 `SessionStore`，SQLite 对象同时实现两者。`worker_launches` 启动 fence（namespace / launchId / artifact，registered 时附带 rootPid / pgid）。reserve 发生在原生 Session 创建前，故不持有 sessions 外键；登记和清除均按原 launchId CAS。它不赋予重启 Host 向旧 PID 发信号的权限。`recoverUnavailable` 在无法准入原生会话时原子保留旧身份与普通队列、冻结未决派发及旧回调。列表包括 active / recovery_required，recovery 排除 acknowledged / cancelled。schema 版本由上文的数据库身份规则统一声明，直接协同替换，无迁移或旧格式兼容分支。
