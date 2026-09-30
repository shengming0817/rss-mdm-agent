# 双平台安全服务实验室操作

范围见[架构](../architecture/local-service.md)。本指南是验收入口，不是通过证明。需要可重置的 macOS 26.4 arm64 / Windows 11 x64。当前开发机缺少 MinGW C 编译器，完整 Windows 桌面构建及真实服务安全矩阵需对应环境补齐。

## 构建与数据

使用根 manifest 和工具链文件指定的版本，先安装冻结依赖。执行：

    pnpm build:ai-access
    pnpm desktop:build
    cargo build --release --locked -p local-service

桌面构建选择 macOS app / Windows NSIS 配置，携带当前平台运行包；Windows 同时产出 .local-ci-runs/windows-lab-desktop，管理员安装只消费这个带摘要清单的展开目录。Windows 从 pnpm 环境运行脚本，安装 C/C++ 与 WebView2 构建先决条件。候选包不代表完成发布签名或平台验收。

不支持的旧数据库格式不做迁移。使用全新的实验室 OS 用户/应用数据目录；旧数据保留，旧格式打开应明确失败。不得删除原账户数据来通过测试。

## 管理员安装

macOS：

    sudo python3 scripts/service/install-macos.py --desktop 'target/release/bundle/macos/RSS MDM Agent.app' --service target/release/rss-local-service --probe target/release/rss-untrusted-service-probe --allow-user <普通用户UID>

脚本创建非登录服务账号、固定 app/服务与 LaunchDaemon，采用 ad-hoc cdhash 固定实验室产物，无正式发布者身份承诺。已有 app 要求操作者先处理旧实验室安装；不自动删除旧 app 或恢复旧信任清单。

macOS 固定桌面安装于 `/Library/Application Support/RSS MDM Agent/desktop/RSS MDM Agent.app`；普通用户从该位置打开。其祖先目录均须 root 拥有且不可被组或其它用户写入，不使用通常带 admin 组写权限的 `/Applications`。

Windows 管理员 Windows PowerShell 5.1（Desktop edition，使用原子创建目录 ACL 的 .NET Framework API）：

    .\scripts\service\install-windows.ps1 -DesktopDirectory '.\.local-ci-runs\windows-lab-desktop' -ServiceExecutable '.\target\release\rss-local-service.exe' -ProbeExecutable '.\target\release\rss-untrusted-service-probe.exe' -AllowedUserSid '<普通用户SID>'

脚本注册虚拟服务账号，设置目录与查询 ACL。固定 SHA-256 来自管理员安装产物；不以未验证的签名链代替固定映像身份。

获准普通用户打开桌面设置中的“本机安全服务”，应显示已连接/statusOnly。运行 `node scripts/verify-local-service.mjs`；verifier 从当前 checkout 的有效 Cargo target 目录读取已构建的 `release/rss-local-service`（默认 `target/release`，Windows 带 `.exe`），再读取安装 policy。原生代码从 OS 获取固定位置，验证目录链与文件权限后读取策略，并核验产品授权的映像身份；JS 随后执行正向查询、独立进程负例与再次正向查询。调用者不能另传 policy 或程序路径；helper 缺失或任一步失败均写入失败结果，不回退到 JS 直接读取。回执记录实际 policy 路径、运行版本和权限结果。

policy 来源修复以自动化回归和 Windows target 编译检查交付，不要求 Windows 实机验收作为该修复的完成门槛。下面的平台实验仍可按需运行；未执行的场景不得记为通过。

## 撤销与卸载

    sudo python3 scripts/service/manage-macos.py --revoke-user <UID>
    sudo python3 scripts/service/manage-macos.py --uninstall
    .\scripts\service\manage-windows.ps1 -RevokeSid '<SID>'
    .\scripts\service\manage-windows.ps1 -Uninstall

撤销后旧连接和新查询都应失败。卸载只移除服务注册，保留安装文件和 AI 数据。不回滚业务效果，不宣称 provider 密钥已撤销。

## 必须补齐的平台验收

每个场景记录 OS/架构、安装产物身份、OS 主体、命令、原始响应和 handler 是否获准进入。账号、机器绝对路径与秘密保留在忽略的本地回执，不提交仓库。

| 场景         | 实施入口/检查                                                        | 预期                               |
| ------------ | -------------------------------------------------------------------- | ---------------------------------- |
| 双向身份     | 已安装桌面查询；假服务端；替换客户端/服务文件                        | 仅正确双方可查询                   |
| 独立恶意进程 | 正向查询已通过后运行 rss-untrusted-service-probe；另一个普通用户访问 | 未获准，不能把服务未安装当拒绝证明 |
| worker 越权  | worker 对同一端点发起查询                                            | 缺少桌面进程身份，拒绝             |
| 重放与到期   | 核心套件先通过；真实 IPC 重发 challenge、重复请求、跨连接、超过五秒  | handler 不执行或同连接最多一次     |
| 撤销/重启    | 保持旧连接并运行维护命令或重启，再提交旧请求                         | 旧连接失效，撤销主体不能重连       |
| Windows 边界 | 远程 pipe、错误 SID/session、同名端点、弱 ACL/reparse                | 拒绝，不降级为匿名或 TCP           |
| macOS 边界   | 错 cdhash、同 UID 其它映像、错误 audit session、替换路径             | 拒绝，不只依赖 UID                 |
| 桌面/Codex   | 普通用户窗口、状态查询、manifest 固定的 Codex 对话、取消/关闭           | UI/进程回执分开，保留 S1 标识      |
| OS 生命周期  | 杀 Host/launcher、保留后代、登记失败、关闭超时、旧 scope 不明        | 阻断不确定恢复；终止有范围为空证据 |

cargo test -p local-service、Host/desktop 测试及交叉编译只是前置证据。真实矩阵未完成时不能宣称双平台安全验收通过。

策略格式直接替换，不读取旧策略或自动迁移。升级实验室候选时先卸载服务注册，由管理员处理旧安装文件，再构建并重新安装；保留原 AI 数据，不通过清库规避格式拒绝。

## 后台授权执行服务

构建 `cargo build -p agent-service --bin rss-execution-service`。生产服务与 user helper 使用同一二进制；原 execution-runner 二进制装配已删除。系统服务独占凭据和执行 journal，helper 不初始化业务数据库。

由管理员准备受保护的部署 JSON：macOS 默认 `/Library/Application Support/RSS MDM Agent/execution.json`，Windows 为 ProgramData 下同一产品目录的 `execution.json`。格式由 `apps/agent-service/src/deployment.rs` 持有，显式配置 version、HTTPS origin、tenant、signing_keys、enrollment、registration_operation、state_root、service 产物 pin、clients OS 主体/程序 pin、execution 的 work_root/material_root/interpreters/managers/processes，以及每个已登记用户的 helper_work_roots。解释器和包管理器使用完整路径和固定摘要，不使用 PATH 查找。material_root 与 state_root 分离。

首次注册由系统账号运行 `rss-execution-service --config <受保护配置> --initialize`，通过标准输入传入 enrollment secret；秘密不得放入 argv、JSON、日志或 shell 历史。注册重试使用同一配置、注册操作和 OS 秘密。初始化遇到已有不支持的数据库或缺失的原注册状态时拒绝，保留原文件；不能换目录、清库或重新注册来绕过未决任务。

macOS 系统注册：`python3 scripts/service/execution-macos.py install --scope system --binary <root 拥有的绝对产物路径> --config <受保护配置>`，需要管理员执行。用户在自己的 GUI 登录中运行同一安装脚本，改为 `--scope user`；程序参数固定为 `--user-helper`，只注册当前实际 OS 会话。`--config` 可省略以使用默认部署路径。状态查询用 `rss-execution-service --config <配置> --query`，客户端和服务必须互相匹配真实 OS 身份与程序 pin。

Windows 使用 PowerShell 7 运行 `scripts/service/execution-windows.ps1 -Action Install -Scope System -Binary <绝对 exe 路径> -Config <受保护配置>`，SCM 使用 LocalSystem。用户 helper 用 `-Scope User`，调度任务以实际登录账号启动。跨会话不继承系统进程句柄，使用认证命名管道派发。

卸载将 install/Install 改为 remove/Remove，并提供原始精确二进制和配置路径。已有注册拒绝覆盖；卸载核对归属，只移除注册，不删除凭据、journal、材料或审计。生产桌面读取默认受保护部署 pin；自定义配置用于显式命令行部署和隔离验收，不作为桌面失败后的回退。

本地执行 V5、IPC V5、SQLite schema 6 拒绝旧格式；远程 Agent V4 保持不变。测试执行器仅用于测试专用装配。macOS 实际运行证据与 Windows 编译结果分别记录，未执行的环境不记为通过。

软件脚本检测器在退出码为 0 且完整捕获 stdout 时读取一个严格 JSON 对象：`{"kind":"absent"}` 或 `{"kind":"present","version":"固定版本"}`。其它输出、截断或无法核实的执行活动保持 Unknown；检测事实与安装进程退出分别记入同一 journal。

执行宿主机制诊断：macOS 可用 `log show --last 10m --predicate 'subsystem == "com.rss-mdm.agent.execution"'` 查看闭合的阶段/失败分类；Windows 在 Application Event Log 查看 source 为 `RSS Execution` 的事件数据（不要求自定义消息资源安装）。日志由 OS 留存，卸载不删除历史。launchd 初始化结果不确定会尝试 bootout；补偿失败保留 plist，需核对 endpoint 后重试 Remove，不直接删配置冒充回收完成。

受控 macOS 接线验收先构建 `agent-service` 的 `rss-execution-service` 和 `controlled-backend` example，再运行 `python3 scripts/service/verify-execution-macos.py --binary <构建产物> --backend <example产物> --output <不存在的本地回执目录>`。入口通过原生管理员授权安装隔离配置，使用真实 HTTPS、系统 Keychain、launchd IPC、系统/用户脚本和固定 PKG；只卸载本次注册，保留凭据、journal、材料和包收据供核查。已有执行服务或 helper 注册时拒绝替换。回执的失败或缺失不能算通过。
