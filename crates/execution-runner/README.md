# 平台执行机制

`NativeRunner` 消费 `execution-app::AuthorizedDispatch`，使用固定的本地产物清单执行 V2 计划。清单按完整计划摘要索引；下载、身份、批准和生产装配由对应产品入口持有。`rss-execution-service` 当前装配 `Unbound`，所有执行请求拒绝，没有测试/环境变量回退。

`OsIdentity` 按明确的 OS 账号权限运行，不声明网络或文件沙箱；`Restricted` 无法强制时拒绝。解释器/profile、原始内容摘要、参数、环境、输入、输出结构及累计预算均来自冻结计划。调用方必须提供受保护的普通文件及工作目录；不通过 PATH 寻找解释器，不按扩展名更换运行时。

macOS 支持固定 sh/Bash 调用和固定 osquery 版本查询。系统上下文只在 root 宿主可用，用户上下文由该真实 UID 的用户 helper 执行；要求活动会话时额外检查当前控制台用户。launchd 用户 helper 与系统宿主共用实现，用户断连不会自行取消任务。生产身份、IPC 对端信任策略、批准与企业接线留给 #2564。

每个 attempt 的进程组由专用 watchdog 持有，服务异常退出关闭控制管道并触发组清理。进程组只能控制合作进程：逃逸后代可能仍在运行，因此组不存在和根进程退出都不产生完整静止证明。原始输出、退出状态、输出质量与独立效果事实分开；不生成通用“效果已满足”结论。

原始有界输出保存在既有 execution SQLite 的 attempt 证据中；summary、审计和日志不复制输出。SQLite 仅接受当前格式，旧文件保留并拒绝，不执行迁移、重建或自动重新派发。runner 内存丢失后不合成进程历史。

运行 `cargo test -p execution-runner` 验证本机机制；测试直接调用私有机制，不代表生产授权。launchd/XPC 操作见[本机服务实验指南](../../docs/guides/local-service-lab.md)。Windows adapter 在 #2475 交付；不支持的平台在启动前拒绝。

实现参考：Tokio `tokio/src/process/unix/mod.rs@tokio-1.43.0`（child/pipe 生命周期）、Foundation `NSXPCConnection.h`（连接和原生对端事实）、PowerShell `CommandLineParameterParser.cs@411d5fee10110d9881a909804f9d4eb1a06052ea`（固定 file 调用约定）。
