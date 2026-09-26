# 平台执行机制

`NativeRunner` 消费 `execution-app::AuthorizedDispatch`，使用固定的本地产物清单执行 V2 计划。清单按完整计划摘要索引；下载、身份、批准和生产装配由对应产品入口持有。`rss-execution-service` 当前装配 `Unbound`，所有执行请求拒绝，没有测试/环境变量回退。

`OsIdentity` 按明确的 OS 账号权限运行，不声明网络或文件沙箱；`Restricted` 无法强制时拒绝。解释器/profile、原始内容摘要、参数、环境、输入、输出结构及累计预算均来自冻结计划。macOS 解释器和内容必须来自 root 拥有、目录链不可由运行用户改写的固定安装/cache；脚本以已打开只读 FD 交付，cwd 使用已打开目录 FD。调用方必须提供受保护的普通文件及工作目录；不通过 PATH 寻找解释器，不按扩展名更换运行时。

macOS 支持固定 sh/Bash 调用和固定 osquery 版本查询。系统上下文只在 root 宿主可用，用户上下文由该真实 UID 的用户 helper 执行；要求活动会话时额外检查当前控制台用户。launchd 用户 helper 与系统宿主共用实现，用户断连不会自行取消任务。生产身份、IPC 对端信任策略、批准与企业接线留给 #2564。

每个 attempt 的进程组由专用 watchdog 持有，服务异常退出关闭控制管道并触发组清理。进程组只能控制合作进程：逃逸后代可能仍在运行，因此组不存在和根进程退出都不产生完整静止证明。原始输出、退出状态、输出质量与独立效果事实分开；不生成通用“效果已满足”结论。

原始有界输出以 BLOB 保存在既有 execution SQLite 的 attempt 证据中，仅 ReadAudit 可读取原始 capture。普通结果只含根进程退出、质量、字节数与静止证明摘要，Debug 不打印输出。证据与累计用量持久化后明确 ack 回收 runner 槽位；重放控制仍由同一 SQLite intent 持有。受控输入由可信 resolver 按精确引用/attempt 一次性解析，投递失败不产生 Complete，输入缓冲在释放时清零。SQLite 仅接受当前格式，旧文件保留并拒绝，不执行迁移、重建或自动重新派发。runner 内存丢失后不合成进程历史。

运行 `cargo test -p execution-runner` 验证本机机制；测试直接调用私有机制，不代表生产授权。launchd/XPC 操作见[本机服务实验指南](../../docs/guides/local-service-lab.md)。Windows 原生测试需在 Windows 11 上执行；不支持的平台在启动前拒绝。

实现参考：Tokio `tokio/src/process/unix/mod.rs@tokio-1.43.0`（child/pipe 生命周期）、Foundation `NSXPCConnection.h`（连接和原生对端事实）、PowerShell `CommandLineParameterParser.cs@411d5fee10110d9881a909804f9d4eb1a06052ea`（固定 file 调用约定）。


Windows 使用 SCM LocalSystem 宿主或当前交互用户的 helper；用户计划必须匹配当前 token 的 SID/session，活动会话通过 WTS 检查，不存在用户会话时拒绝。Named Pipe 使用私有 DACL、FIRST_PIPE_INSTANCE、远程客户端拒绝、64 KiB 帧和 5 秒连接期限。两种宿主均驱动同一个 ExecutionApp，默认入口仍为 Unbound。

PowerShell 7 固定 file profile 和 OsqueryInfoV1 共用物化与输出链。解释器/内容要求受信安装账号保护完整路径链，拒绝 reparse/UNC，保留不允许写入/删除共享的句柄。脚本在私有目录写入后重新以只读打开并核对精确内容，完成后删除本次物化文件。标准流使用异步管道；环境显式重建，参数逐项按 Windows argv 规则编码。

Windows 以挂起状态创建目标，PROC_THREAD_ATTRIBUTE_JOB_LIST 在创建时原子绑定禁止 breakaway、kill-on-close 的 Job；确认成员关系后恢复 CreateProcess 返回的原始线程。HANDLE_LIST 仅允许继承三条标准流。结束后核实 Job 活动进程数为零，查询失败保持 Unknown；恢复不按持久化 PID/Job 名称重新获得终止权限。退出零仍不是效果成功。

Windows 测试命令为 `cargo test -p execution-runner`；包含真实 Job 后代、breakaway、owner 丢失和 Named Pipe 机制测试，不要求生产可信入口。PowerShell/管理员安装与真实用户会话需要另行在目标环境验证；交叉编译不能替代这些结果。


Windows transport 与串行 application owner 分线程：连接最多等待 5 秒，超时即停止整个宿主接入并收尾，不重连仍被旧 callback 持有的 pipe instance；排队请求在调用前发现响应端已关闭则不启动 mutation。已经进入 application 的请求失去响应不能解释为未提交，仍以原 request/attempt 查询恢复。owner 5 秒无进展则停止接入，关闭等待最多 3 秒；若 handler 无法收尾，专用宿主以 ERROR_TIMEOUT 退出，禁止 detach 仍持有 Job/SQLite 的 owner。系统失败用 ERROR_PROCESS_ABORTED 上报，日志不含执行 payload。

Windows runner 测试需要受保护安装的 `C:\Program Files\PowerShell\7\pwsh.exe`，可仅在测试进程以 `RSS_TEST_PWSH7` 指定受保护的同版本绝对路径；缺失时测试明确失败，不跳过伪报通过。测试通过 Test carrier 装配实际 NativeRunner，再验证 Job capture 的 SQLite 持久化/ack/重开不重派；该 carrier 只编译进测试二进制。


机制失败通过必填 `ProcessFailureKind` 保存，区分身份/输入绑定、权限或制品拒绝、能力、格式、资源、启动、输入投递、输出读取及监督失败；它不替代 end、quality 或效果观察。首个非空故障只能保留，不能被后续 capture 擦除。缺失此字段的 capture 拒绝读取，不补默认值。桌面将这些闭集及停止/效果状态映射为中文提示。

macOS 工作目录从 `/` 开始逐级 `openat(O_DIRECTORY|O_NOFOLLOW)`，以 fd 校验 owner/mode/ACL，最终 fd 交给 fchdir；目录祖先被替换不会改变已绑定对象。launchd bootstrap 回执失败后先 bootout 补偿；撤销不确定时保留 plist 供操作员重试，不把配置删除当成停止证明。

宿主诊断仅记录闭合 stage、failure 和 system/user 模式：macOS 使用 Unified Logging 子系统 `com.rss-mdm.agent.execution`，Windows 使用 Application Event Log 的 `RSS Execution` source（事件数据含结构化文本，不依赖自定义 message DLL）。两者使用 OS 管理的日志，不创建应用日志文件或安装注册项，卸载保留 OS 历史记录。Windows [RegisterEventSourceW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-registereventsourcew) 的未注册 source 按官方语义写入 Application；macOS 使用 SDK `os/log.h` 的 error 级日志。不记录参数、路径、PID、输出或凭据。
