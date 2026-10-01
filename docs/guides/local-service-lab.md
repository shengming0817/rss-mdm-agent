# 唯一执行服务的部署与实验室验收

生产组合根为 agent-service，桌面状态和业务任务使用同一认证 IPC。安全机制由 installation-security 持有；用户 helper 只执行原登录会话中的物理操作。

## 后台授权执行服务

构建 `cargo build -p agent-service --bin rss-execution-service`。生产服务与 user helper 使用同一二进制；原 execution-runner 二进制装配已删除。系统服务独占凭据和执行 journal，helper 不初始化业务数据库。

由管理员准备受保护的部署 JSON：macOS 默认 `/Library/Application Support/RSS MDM Agent/execution.json`，Windows 为 ProgramData 下同一产品目录的 `execution.json`。格式由 `apps/agent-service/src/deployment.rs` 持有，使用当前格式显式配置 version、ipc_version、HTTPS origin、tenant、signing_keys、enrollment、registration_operation、state_root、service 产物 pin、clients OS 主体/程序 pin、execution 的 work_root/material_root/interpreters/managers/processes，以及每个已登记用户的 helper_work_roots。解释器和包管理器使用完整路径和固定摘要，不使用 PATH 查找。material_root 与 state_root 分离。

首次注册由系统账号运行 `rss-execution-service --config <受保护配置> --initialize`，通过标准输入传入 enrollment secret；秘密不得放入 argv、JSON、日志或 shell 历史。注册重试使用同一配置、注册操作和 OS 秘密。初始化遇到已有不支持的数据库或缺失的原注册状态时拒绝，保留原文件；不能换目录、清库或重新注册来绕过未决任务。

macOS 系统注册：`python3 scripts/service/execution-macos.py install --scope system --binary <root 拥有的绝对产物路径> --config <受保护配置>`，需要管理员执行。用户在自己的 GUI 登录中运行同一安装脚本，改为 `--scope user`；程序参数固定为 `--user-helper`，只注册当前实际 OS 会话。`--config` 可省略以使用默认部署路径。状态查询用 `rss-execution-service --config <配置> --query`，客户端和服务必须互相匹配真实 OS 身份与程序 pin。

Windows 使用 PowerShell 7 运行 `scripts/service/execution-windows.ps1 -Action Install -Scope System -Binary <绝对 exe 路径> -Config <受保护配置>`，SCM 使用 LocalSystem。用户 helper 用 `-Scope User`，调度任务以实际登录账号启动。跨会话不继承系统进程句柄，使用认证命名管道派发。

卸载将 install/Install 改为 remove/Remove，并提供原始精确二进制和配置路径。install 拒绝已有注册；卸载核对归属，只移除注册，不删除凭据、journal、材料或审计。生产桌面读取默认受保护部署 pin；自定义配置用于显式命令行部署和隔离验收，不作为桌面失败后的回退。

本地执行 V5、桌面/系统 IPC V6、SQLite schema 6 拒绝旧格式；远程 Agent 使用 V5，通信 SQLite 使用 schema 3。远程旧协议和旧通信库明确拒绝，保留原文件，不迁移、自动换库或重新注册。测试执行器仅用于测试专用装配。macOS 实际运行证据与 Windows 编译结果分别记录，未执行的环境不记为通过。

软件脚本检测器在退出码为 0 且完整捕获 stdout 时读取一个严格 JSON 对象：`{"kind":"absent"}` 或 `{"kind":"present","version":"固定版本"}`。其它输出、截断或无法核实的执行活动保持 Unknown；检测事实与安装进程退出分别记入同一 journal。

执行宿主机制诊断：macOS 可用 `log show --last 10m --predicate 'subsystem == "com.rss-mdm.agent.execution"'` 查看闭合的阶段/失败分类；Windows 在 Application Event Log 查看 source 为 `RSS Execution` 的事件数据（不要求自定义消息资源安装）。日志由 OS 留存，卸载不删除历史。launchd 初始化结果不确定会尝试 bootout；补偿失败保留 plist，需核对 endpoint 后重试 Remove，不直接删配置冒充回收完成。

受控 macOS 接线验收先构建 `agent-service` 的 `rss-execution-service` 和 `controlled-backend` example，再运行 `python3 scripts/service/verify-execution-macos.py --binary <构建产物> --backend <example产物> --output <不存在的本地回执目录>`。入口通过原生管理员授权安装隔离配置，使用真实 HTTPS、系统 Keychain、launchd IPC、系统/用户脚本和固定 PKG；只卸载本次注册，保留凭据、journal、材料和包收据供核查。已有执行服务或 helper 注册时拒绝替换。回执的失败或缺失不能算通过。

材料目录当前最多 8 GiB / 32,768 条目，达到配额时拒绝新材料，重启不清理空间。自动安全回收由 #2588 跟踪；管理员不得清空材料目录或 journal 来绕过未决任务。


### 已发布 SQL 模板采集

生产配置的 `execution.interpreters` 可以登记 `osquery` profile，映像仍使用受保护的完整路径和固定 SHA-256。Agent 签入声明本次配置中的执行器，服务端只下发可执行的模板。SQL 文本取自固定摘要的资源文件，签名任务仅带模板身份、冻结参数和预算；没有临时 SQL 输入入口。参数通过共享 AST 转换成字面量，Agent 再次检查单表 SELECT、表/列/函数许可及平台。调用固定关闭扩展、事件、分布式查询、数据库和外部 flagfile，并设置一行溢出检测。

脚本和 SQL 结果继续通过任务 journal 与通信 SQLite 补传；大输出拆成 256 KiB 的固定块，总量不超过 16 MiB。只有全部块和整体摘要匹配后才提交终态结果。丢失响应重放原操作，重启不重新执行查询，也不追加客户端批准。

真实 osquery 接缝使用 `OSQUERY_TEST_BINARY=<已独立验证的官方二进制绝对路径> python3 scripts/build-run.py -- cargo test --locked -p execution-runner --test osquery -- --ignored`。macOS 必须保留官方 `.app` 完整结构以验证签名，单独复制 Mach-O 文件会破坏签名。该测试验证固定参数、字面量绑定和成功零行；不替代系统服务身份或完整平台部署验收。可复现候选为官方 5.23.1 的 `osquery-5.23.1_1.macos_arm64.tar.gz`，SHA-256 为 `5484f0b62e05a7b2fa9d6e43f038915ea2b7ce063d59bd671ede0cf8dd0552da`。

执行上下文使用本机数值 OS 版本和认证 helper 的登录世代，待确认注册与 claim 保留原上下文。#2531 合入后的软件合同按格式/作用域声明能力；当前远程生产编译器消费 MSI、PKG 和 Bundle 已实现路径，拒绝尚未实现的 EXE/DMG/MSIX 及新版 WinGet/Brew 原生发布源，不宣称这些 profile 可执行。既有本地 runner 的冻结任务恢复不变。原生返回码与升级调用若超出当前 runner 已实现语义，同样明确拒绝，不回退为脚本或猜测命令。

## 显式开发刷新

源码构建不会更新已安装服务。先将新服务、桌面及 helper 候选暂存到管理员保护的位置，完成签名/摘要和当前格式配置核验，再刷新已核对归属的注册。不能把用户可写 worktree 产物直接加入生产 pins。

macOS 系统操作由管理员运行，用户 helper 操作必须在原用户 GUI 登录中运行：

```sh
python3 scripts/service/execution-macos.py refresh --scope system --binary <原受保护产物> --config <原当前格式配置> --candidate-binary <新受保护产物> --candidate-config <新当前格式配置>
```

Windows 使用同一 owner：`execution-windows.ps1 -Action Refresh -Scope System -Binary <原产物> -Config <原配置> -CandidateBinary <新产物> -CandidateConfig <新配置>`；当前用户 helper 改为 `-Scope User`。

刷新只替换匹配的注册候选，保持原身份、凭据、journal、材料和 helper 工作目录；不转换旧配置、不初始化或重新注册。停止、替换或重启失败时保留可检查的注册，不能解释为完成。脚本返回 registered 仅证明注册操作，随后需获准普通用户启动固定桌面并运行 `--service-probe`，核实认证连接与 readiness。若尚未注册，诊断可连接，但任务接口仍拒绝。

## 原生开发与验收

`pnpm dev --fixture` 为持续人工操作入口，复用已有 fixture，完全不安装系统服务。`pnpm check:desktop-native --visual` 使用相同装配自动验证真实 WKWebView。

真实旅程使用显式 `pnpm check:desktop-native --controlled-service`：复用本指南受控后端及安装 owner，构建固定候选、请求原生管理员授权、暂存受保护桌面并由实际登录用户启动。已有执行注册/helper 或默认 deployment 时拒绝替换；普通 `pnpm dev` 不执行此安装操作。真实验收从认证 IPC 和服务 journal 回读请求，取消、进程终止与设备效果分别取证。

旧独立状态服务已退出。实验室管理员如发现历史注册，须先核对真实程序、参数和安装归属，再使用 OS 原生管理工具停用/撤销注册；本仓不再提供旧服务安装或兼容接口。不得为迁移删除历史凭据、journal、个人数据或覆盖未知安装。

macOS arm64 的实际执行、Windows 实现/编译以及 #2559 双平台真实安全矩阵分别记录。fixture、服务未安装或单平台结果不能替代后两项。正式 MSI/PKG、签名、公证和首次生产注册仍由其原 owner 持有。
