# 唯一执行服务的部署与实验室验收

生产组合根为 agent-service，桌面状态和业务任务使用同一认证 IPC。安全机制由 installation-security 持有；用户 helper 只执行原登录会话中的物理操作。

开发机按[验证规则](../rules/verification-scope.md#开发机平台验证与实现项关闭)只运行当前宿主平台的必要验证，不执行 Windows 编译、测试、打包、安装或真机验收，也不补齐 Windows 环境。下文 Windows 部署和诊断操作仅供目标 Windows 环境按需使用；未执行时记录为未验证，不阻塞已完成实现项的关闭。

## 固定安装包与首次注册

`pnpm build:installer --organization /absolute/public-organization.json` 在当前原生平台构建 macOS arm64 PKG 或 Windows amd64 / arm64 的 perMachine NSIS EXE，不套用 MSI。macOS 构建命令通过 `python3 scripts/build-run.py -- pnpm build:installer ...` 复用本地 Cargo 槽位；Windows 使用 `pnpm exec node scripts/build-installers.mjs ...`。Windows 构建进程、Rust 工具链和 Node 必须为目标原生架构，不接受仿真进程或混合 PE。目标设备须预装 machine-wide WebView2；安装器缺失时明确拒绝。当前机器有匹配架构、受保护的 PowerShell 7 时才登记该解释器与相关能力。

组织输入使用当前格式的 `service_id`（后端安装 instance_id）、`tenant`、`connections`（后端下载的完整签名 Agent 连接配置）、`label`、`signing_keys`（安装时独立保护的注册/配置验签公钥）与必填 `ca_pem`（HTTPS bootstrap CA）。打包器把连接文件交给生产 Rust 校验器验签和核对身份，不在 Node 重写 wire 验证器。设备 CA/CRL 经已授权 Agent HTTPS 入口获取；任务公钥由已认证设备通道提供，独立于安装公钥。桌面登录和会话仍使用根 `.env` 的 `RSS_MDM_ORIGIN`、租户与可选 `RSS_MDM_CA_FILE`，通过登录会话中的稳定实例与租户核对机器身份，地址变化不授权披露网站 Cookie。Agent bootstrap CA 不用于网站，签名地址更新不扩大 CA 或软件源凭据权限。输入不允许 grant、secret、账号目录或私钥。包内固定生产服务、桌面、Node、AI Host、worker 和资源；安装文件与 `package-manifest.json` 位于忽略的 `.local-ci-runs/installers/`。上传后端仅需要实际安装文件；本地资源清单用于部署与代码身份完整性。

默认 macOS 为 ad-hoc 开发签名，Windows 为明确标记的开发包。正式包增加 `--release`：macOS 设置 `RSS_MACOS_APP_SIGNING_IDENTITY`、`RSS_MACOS_INSTALLER_SIGNING_IDENTITY` 和已存在的 `RSS_MACOS_NOTARY_PROFILE` Keychain 引用；Windows 设置 `RSS_WINDOWS_SIGNING_THUMBPRINT`，可选 `RSS_SIGNTOOL` 完整路径。构建不会接收证书密码；正式 macOS 包完成 notarization / stapling，正式 Windows 包与原生资源进行 Authenticode 签名。只有显式请求 `--release` 时才要求这些正式签名材料；默认包可正常上传、发布和注册，无测试/试点/生产环门槛。

默认部署验收是首次安装、首次注册；重复验证全新安装时使用下述显式 SIT 清理脚本。每个安装包使用独立版本目录。检测到既有部署时会进入已有升级校验；凭据不可读或候选活动未确认停止时拒绝切换，不清库、重注册或自动放宽钥匙串授权。旧格式配置不迁移或兼容读取。

无配置的固定安装包可以匿名下载并保持未注册。安装后在 Agent 中选择“登录并注册此设备”，复用已有企业会话或通过原生密码输入登录；注册授权由桌面向网站业务 API 发起，设备密钥与注册进度由系统服务持有。

管理员提供的完整签名文件命名为 `rss-agent-enrollment.json`，与原始 EXE／PKG 放在同一目录。Windows 从原始 EXE 路径、macOS 从 Installer 传入的外层 PKG 路径发现文件。文件使用后端公开 `ConfigurationFile` 格式，包含配置交换秘密；不需要第三个秘密文件。已安装客户端也可通过原生“导入注册配置”选文件。

系统服务验证配置签名、组织、原生目标、期限和发布候选，为本机生成独立密钥/CSR/操作和设备秘密；服务端交换独立 grant 后复用现有签发、mTLS 激活流程。多台设备可以复用同一管理员文件，但不能共享机器私钥或设备 grant。配置到期不退管已激活设备。

```sh
# 管理员部署的补充入口；只传路径，不能把 JSON/secret 放进 argv。
<受保护的服务程序> --installation import --configuration /absolute/rss-agent-enrollment.json
```

旧 Bootstrap envelope、双文件首次注册和 `--initialize` 已退出，不提供兼容解析。安装器发现无效配置时保留安装并报告注册失败，可改用 Agent 登录；结果未知时保留原操作恢复，不通过换身份重试。已有设备身份不因登录、重复导入或重装而替换。

本地 `candidate.json` 只绑定已安装程序、运行资源及受保护组织配置；`package-manifest.json` 记录本机清单与最终安装包摘要。上传发布不消费这些本地清单，也不要求额外证明文件。

安装成功与认证 readiness 分开核实，使用获准的普通用户运行已安装桌面的 `--service-probe`。共享注册文件仍由管理员负责保管和清理，安装器不删除它。

初装从实际互动登录发现主体及 helper 路径；无人登录的静默初装可省略 profile，以空 IPC 用户集合启动机器注册；管理员需要预设桌面访问主体时提供受保护的 profile（`subjects` 与 `helper_work_roots`）。Windows EXE 接受 `/S /PROFILE=绝对路径`；macOS 可先固定 profile 于 `/Library/Application Support/RSS MDM Agent/install-profile.json` 再运行 `installer -pkg ... -target /`。桌面启动请求 Rust owner 注册当前配置用户的 helper，AI Host 随该桌面运行。

设备退管由服务端普通 Policy `unmanage_device` 发起。Agent 验证绑定当前租户、注册、设备和任务的签名 Start，持久接受后关闭新执行准入，读取 journal 的真实静止证明，再将卸载交给独立安装 worker。离线、证书故障和 MDM Profile 消失不构成卸载授权。产品 `uninstall` 和 system `remove` 入口必须携带 `--operation <已接受任务 UUID>`；仅传 UUID 不授予权限，缺少原 Start 或排空证明均拒绝。Windows NSIS 可传 `/RETIREMENT=<任务 UUID>`，MSI 对应 `RSSRETIREMENTTASK`；普通系统卸载入口不会隐式申请退管。

受控卸载移除本安装服务和程序，按本安装凭据创建记录清理精确 Keychain/CNG/DPAPI 引用；首次初始化持有完整目录标记，旧安装缺少此标记时不能证明历史轮换凭据全部清理，删除已知引用后持久记录不完整证明并上报 Unknown，不永久重试，不自动回填或扩大为全机扫描；升级替换仍保留 installation ID、凭据和原任务。完成清理后删除本安装运行状态、执行材料和 work 目录，重新安装必须经新的注册授权，不继承旧任务。卸载先在 payload 外保留固定摘要的恢复程序及受保护 `install-operations/<任务 UUID>.retirement-work.json` 交接，逐阶段记录停止、程序移除、凭据清理、状态清理和载体最终回收；恢复不依赖已经删除的通信库或程序目录。恢复持有安装锁并核对当前安装身份，不能清理后来复用同一路径的新安装。launchd/SCM 对未完成的同一任务重试；阻塞的桌面应用退出后可继续。主体清理完成后先持久记录，再回收恢复载体；两者有独立完成判据，主体终态落盘后的恢复仍会回收载体。macOS 最终回收由固定参数的系统 shell launchd job 完成，Windows 先持久注册独立 SYSTEM 计划任务（注册与启动触发、固定周期重试），使用系统工具确认精确 SCM worker 已消失后再删除载体 EXE，最后删除自身任务；固定批处理定义与 JSON 一起保留为受保护审计；回收或回执未确认仍保留 Unknown。OS 历史日志、交接与受保护 `install-operations/<任务 UUID>.retirement.json` 安装结果保留。结果通过删除凭据前保留的内存传输提交，不复制长期凭据；响应丢失仍是未确认。当前固定安装 owner 仅对 macOS PKG、Windows NSIS 广告远程退管能力；MSI 缺少固定产品身份的远程包管理器接线，不广告该能力，已有授权的 MSI Remove 仍进入同一清理 owner；MSIX 的 Package Manager 卸载不冒充该安装 owner，保留不支持状态。Windows 独立回收任务接纳后报告 Unknown，交接成功不等于已经删除；真实平台效果由独立 T3 核实。

Windows 安装操作返回 UUID 和不含输入内容的阶段回执。等待超时表示结果未知，worker 可能仍在运行；使用 `--installation operation-status --operation <UUID>` 核实，确认进程退出后回收该 worker。回执保留失败阶段、闭合错误、可得的原生退出码和补偿结果；超时后不要据命令退出自行删除正在使用的程序。

需要在开发 Mac 重复验证全新安装时，使用 `sudo python3 scripts/service/reinstall-macos-sit.py --package /absolute/agent.pkg --sha256 <发布包摘要> --tenant <SIT租户UUID>`。加 `--plan` 可先查看清理范围。脚本校验已安装的开发包与目标租户，停止本产品进程，删除该安装名下的设备 Keychain 条目、本地注册和桌面状态，再调用系统安装器安装校验过的 PKG；不归档，不删除其他应用凭据或服务端记录。清理后按首次注册流程生成新设备身份，不能把它记录为原身份升级验证。

## 后台授权执行服务

构建 `cargo build --release -p agent-service --bin rss-execution-service`。生产服务与 user helper 使用同一二进制；原 execution-runner 二进制装配已删除。系统服务独占凭据和执行 journal，helper 不初始化业务数据库。

由管理员准备受保护的部署 JSON：macOS 默认 `/Library/Application Support/RSS MDM Agent/execution.json`，Windows 为 ProgramData 下同一产品目录的 `execution.json`。格式由 `apps/agent-service/src/deployment.rs` 持有，使用当前格式显式配置 version、ipc_version、service_id、签名 connections、tenant、signing_keys、installation_id、installation_artifact、ca_file（受保护的 HTTPS bootstrap CA）、registration_clients 桌面程序/用户 pin、state_root、service 产物 pin、clients OS 主体/程序 pin、execution 的 work_root/material_root/interpreters/managers/processes，必填 software_read_grants（不使用源凭据时为 []），以及每个已登记用户的 helper_work_roots。解释器和包管理器使用完整路径和固定摘要，不使用 PATH 查找。material_root 与 state_root 分离。

桌面使用根 `.env` 的可选 `RSS_MDM_CA_FILE`；后台 Agent 必须配置独立的 `RSS_AGENT_CA_FILE`，指向部署方明确提供的单个 HTTPS bootstrap CA 公共证书。桌面构建嵌入其公共证书，Agent 的 `execution.json.ca_file` 由 Agent 输入生成。生成新安装配置时先准备包含程序 pins、工作目录、注册身份等完整字段的模板，然后执行：

```sh
node scripts/agent-organization.mjs --config /private/execution-template.json --output /private/execution.json --ca-output /private/organization-ca.pem
```

`--ca-output` 是 CA 在设备上的最终绝对路径，生成器写入同一公共证书并记录该路径；Agent 必须提供该参数。输出配置与 CA 为 0644（公开信息），安装后的父目录须允许用户 helper 遍历、文件由管理员持有且普通用户不可写；凭据和状态目录保持原有保护。输出文件必须不存在，不覆盖既有部署；保持模板的其它字段，仅替换组织地址、租户和 CA。按原安装要求将配置与证书置于管理员保护的最终路径，再进行首次注册。部署不同设备时使用目标设备对应的绝对路径。现有设备不要通过生成新身份或重新注册来更新 CA；沿用原身份，按受控刷新流程发布新配置。更新 CA 后桌面重新构建、Agent 重新加载；HTTPS bootstrap 配置不改系统信任；设备 CA 和 CRL 由 Agent PKI owner 在私有通信存储中应用，不安装系统全局根。CA 私钥不分发，任务签名公钥与组织 HTTPS CA 分开管理。

首次注册使用上文的 Agent 内企业登录或完整签名配置。机器请求、CSR、公钥引用与签发/激活操作由系统服务持久保存；重试使用原操作。人工换证仍保持原 Registration、存储位置和 journal。初始化遇到不支持的数据库或缺失的原状态时保留原文件并拒绝，不通过清库或换目录绕过未决任务。

macOS 系统注册：`python3 scripts/service/execution-macos.py install --scope system --binary <root 拥有的绝对产物路径> --config <受保护配置>`，需要管理员执行。用户在自己的 GUI 登录中运行同一安装脚本，改为 `--scope user`；程序参数固定为 `--user-helper`，只注册当前实际 OS 会话。`--config` 可省略以使用默认部署路径。状态查询用 `rss-execution-service --config <配置> --query`，客户端和服务必须互相匹配真实 OS 身份与程序 pin。

Windows 使用 PowerShell 7 运行 `scripts/service/execution-windows.ps1 -Action Install -Scope System -Binary <绝对 exe 路径> -Config <受保护配置>`，SCM 使用 LocalSystem。用户 helper 用 `-Scope User`，调度任务以实际登录账号启动。跨会话不继承系统进程句柄，使用认证命名管道派发。

隔离验收使用 `remove-lab-registration`（PowerShell wrapper 为 `RemoveLabRegistration`），只允许非默认配置且原生注册参数精确匹配本次 config/binary；保留凭据、程序和 journal，不授予生产退管能力。用户 helper 的 remove 只移除该用户自己的匹配注册；生产 system remove 必须消费上述已接受退管任务，不能作为产品卸载的未授权旁路。安装升级内部的服务替换与补偿不使用产品退管入口。install 仍拒绝已有注册。生产桌面读取默认受保护部署 pin；自定义配置仅用于显式部署和隔离验收。

自助目录的准入主体由后端返回：设备准入任务无需企业登录，企业用户准入任务需要当前有效企业会话。企业登录不授予本机管理员权限；系统账户与本机登录用户的执行身份另行核验。退出或切换企业账号后，目录、参数表单和旧 AI 入口失效；已接受的设备请求保持原 UUID、OS 会话、AI 来源和人工确认，可继续查询或恢复，企业用户请求不能沿用已撤销的企业会话。OS 会话变化仍禁止向另一会话派发。

本地执行、桌面/系统 IPC、执行 journal 和通信库按各自代码持有的格式版本拒绝旧数据；远程 wire 由固定 backend artifact 持有。远程旧协议和旧通信库明确拒绝，保留原文件，不迁移、自动换库或重新注册。测试执行器仅用于测试专用装配。按实际运行的平台分别记录编译、测试与验收结果；开发机不承担 Windows 验证，未执行的环境不记为通过。

本次合同切换仅支持新部署。旧节点先由旧服务完成任务、清理与结果补传；未决事实保留并阻止对应节点切换。旧库即使无未决任务也不提供原地 schema 迁移，保留原部署配置、身份、OS 密钥/Keychain、凭据与 journal；新环境使用独立状态目录和空库显式注册 V1。不得自动换库、清库或重注册原节点。

软件脚本检测器在退出码为 0 且完整捕获 stdout 时读取一个严格 JSON 对象：`{"kind":"absent"}` 或 `{"kind":"present","version":"固定版本"}`。其它输出、截断或无法核实的执行活动保持 Unknown；检测事实与安装进程退出分别记入同一 journal。

### 本地软件操作规则

当前安装文档格式为 deployment 3，桌面/服务 IPC 为 3；旧格式直接拒绝，不协商或回退。显式安装为 `execution.local_software.policyFile` 建立管理员保护的 `software-policy.json`，正常刷新保留已有有效规则。运行时缺失、权限错误或格式错误均禁止本地操作，不自动创建允许规则。修改规则使用已有管理员部署操作；不要修改执行日志或创建第二个执行服务。

初始规则内容如下；它允许支持的非保护目标进入准备，实际提交仍重新核验当前规则、原 OS 会话、观察快照与对象身份：

```json
{"version":1,"allowNativeUninstall":true,"allowLaunch":true,"allowUserUpdate":true,"protected":["com.rss.mdmagent","com.rss-mdm.agent","RSS MDM Agent"],"recipes":[]}
```

`protected` 为明确原生标识，比较忽略 ASCII 大小写；系统 Apple 标识和本产品身份始终受保护。`allowNativeUninstall` 控制无配方的原生卸载，明确配方仍需组织单独允许。规则文件受既有 protected reader 大小上限约束。

macOS 自动路径支持 `/Applications` 直属、可核实签名的 `.app` 和当前用户 Applications 中的应用；系统根、别名及设备级嵌套目录不推断卸载权限。系统服务先使用禁止覆盖的原子重命名隔离并核验原目录对象，再执行不跟随链接的删除；失败或中断保留未知结果和隔离内容，不根据旧路径重试删除。GUI 使用 NSWorkspace，在当前用户 helper 内启动。

Windows 原生路径支持经 Windows Installer 与受保护缓存重新核实的 MSI 产品，以及当前用户已注册、非框架、非系统的已签名 MSIX。MSI 的用户/机器安装上下文分别执行；系统组件、禁止移除项、本产品及需要单独配方的 Microsoft 产品拒绝自动卸载。MSIX 移除当前用户注册，不将设备取消预配冒充所有用户卸载。GUI 使用唯一 AppListEntry 的原生启动接口；多入口不猜测。开发机不执行 Windows 构建或验收。

PKG、EXE 和其它不具备通用安全卸载语义的目标要求 `recipes` 中的明确配方，不执行注册表 UninstallString，不根据 PKG 收据遍历删除文件。每个配方必须给出 `nativeId`、`kind`、`scope`（`user` / `device`）、`action`（`update` / `uninstall`）、`publisher`、`fromVersions`、`targetVersion`、既有 `ExecutionInput` 的 `input` 与精确 `SoftwareMaterial[]` 的 `files`。调用者不能传入路径、命令或可执行输入；服务将原请求、用户、设备、执行范围及登录会话绑定进现有 SoftwareProgram，材料必须位于受保护路径并匹配 SHA256。卸载配方匹配当前确切安装版本，且提供独立 detector；检测动作可复用同一卸载配方的只读 detector。

本地更新仅允许当前用户范围，要求 `fromVersions` 明确包含所观察版本、固定 `targetVersion` 等于程序最终版本，并满足既有执行器的前向版本规则；不支持未知顺序或允许降级的候选。设备级更新继续使用企业任务。PKG/EXE 的安装与企业升级仍消费现有发布和执行机制，不由本地规则签发企业授权。

普通企业网络故障或证书过期阻止相应后台工作，不撤销已经独立获准的本地动作；退管、凭据撤销、服务停止仍阻止新的本地提交。规则变更、选择快照变化、准备过期、会话变化及材料变化都必须重新核实，点击确认本身不能补权。

执行宿主机制诊断：macOS 可用 `log show --last 10m --predicate 'subsystem == "com.rss-mdm.agent.execution"'` 查看闭合的阶段/失败分类；Windows 在 Application Event Log 查看 source 为 `RSS Execution` 的事件数据（不要求自定义消息资源安装）。日志由 OS 留存，卸载不删除历史。launchd 初始化结果不确定会尝试 bootout；补偿失败保留 plist，需核对 endpoint 后重试 Remove，不直接删配置冒充回收完成。

受控 macOS 接线验收先以 release profile 构建 `agent-service` 的 `rss-execution-service` 和 `controlled-backend` example，再运行 `python3 scripts/service/verify-execution-macos.py --binary <构建产物> --backend <example产物> --output <不存在的本地回执目录>`。入口通过原生管理员授权安装隔离配置，使用真实 HTTPS、系统 Keychain、launchd IPC、系统/用户脚本和固定 PKG；只卸载本次注册，保留凭据、journal、材料和包收据供核查。已有执行服务或 helper 注册时拒绝替换。回执的失败或缺失不能算通过。

每次受控旅程只请求一次管理员授权，限于预先固定的安装、初始化、重启和清理操作；不保存密码。授权等待超过 120 秒即终止，迟到授权不能创建安装；取消和超时分别保留回执，重新开始旅程才会再次请求授权。

如明确选择本地密码文件，可为两个受控入口提供 `--authorization-password-file <绝对路径>`。文件须属于当前登录用户、仅该用户可读写（`0600`）、为普通文件且非符号链接；不得提交。密码仅通过 AppleScript 标准输入提供给原生授权，不进入 argv、日志、部署配置或回执。验收不创建或复制密码；`.passwd` 已由 Git 忽略。

材料目录最多 8 GiB / 32,768 条目，硬链接按同一文件身份计费，每个目录和文件名占一个条目。系统服务独占材料目录，并绑定设备权威及原 journal 文件身份；不匹配时拒绝打开，不覆盖归属。服务在提交之间完整读取 journal 与补传状态，只回收已安全结清且无活跃引用的精确材料；Unknown、补传未完成、未知来源和无法核实的材料保留。达到配额时拒绝新材料。管理员不得清空材料目录、删除归属文件或替换 journal 来绕过未决任务；新部署使用独立材料目录。

### 已发布 SQL 模板采集

生产配置的 `execution.interpreters` 可以登记 `osquery` profile，映像仍使用受保护的完整路径和固定 SHA-256。Agent 签入声明本次配置中的执行器，服务端只下发可执行的模板。SQL 文本取自固定摘要的资源文件，签名任务仅带模板身份、冻结参数和预算；没有临时 SQL 输入入口。参数通过共享 AST 转换成字面量，Agent 再次检查单表 SELECT、表/列/函数许可及平台。调用固定关闭扩展、事件、分布式查询、数据库和外部 flagfile，并设置一行溢出检测。

脚本和 SQL 结果继续通过任务 journal 与通信 SQLite 补传；大输出拆成 256 KiB 的固定块，总量不超过 16 MiB。只有全部块和整体摘要匹配后才提交终态结果。丢失响应重放原操作，重启不重新执行查询，也不追加客户端批准。

真实 osquery 接缝使用 `OSQUERY_TEST_BINARY=<已独立验证的官方二进制绝对路径> python3 scripts/build-run.py -- cargo test --locked -p execution-runner --test osquery -- --ignored`。macOS 必须保留官方 `.app` 完整结构以验证签名，单独复制 Mach-O 文件会破坏签名。该测试验证固定参数、字面量绑定和成功零行；不替代系统服务身份或完整平台部署验收。可复现候选为官方 5.23.1 的 `osquery-5.23.1_1.macos_arm64.tar.gz`，SHA-256 为 `5484f0b62e05a7b2fa9d6e43f038915ea2b7ce063d59bd671ede0cf8dd0552da`。

执行上下文使用本机数值 OS 版本和认证 helper 的登录世代，待确认注册与 claim 保留原上下文。生产编译器消费当前远程 V1 的 MSI、PKG、Bundle、离线 EXE、MSIX、DMG，以及绑定实际 target/publication 的 WinGet/Brew 原生发布源。EXE 仅执行完整离线布局、显式参数和明确卸载入口；退出码与重启按源任务策略解释，不自动重启。实际启动 EXE 后，当前 V1 没有可信外部安装会话完成协议，因此保留真实退出码与重启事实，同时报告 Unknown、保留离线布局与排他；Job 归零或检测命中不解除该限制。MSIX 明确区分目标用户注册与未来用户设备 provisioning，后者不改写已有用户注册；只查询既有侧载/依赖，不导入证书、改策略或终止应用。DMG 用镜像的完整长度/SHA256 确认内容身份，只读挂载并精确选择 app/内含 PKG，不计算独立 app 目录摘要；app 保留内部链接与 quarantine，经原生签名/Gatekeeper 检查再复制或原子替换。运行中应用拒绝替换/删除；普通 app 卸载直接核对声明的目标、Bundle ID、版本、上下文、签名与架构，不挂载镜像或比较镜像目录；删除使用本次尝试的短期对象快照和私有隔离，失败保留清理记录。卸载不清理用户数据。挂载、staging 和物理阶段在同一 attempt 中核算，Unknown 保留证据与排他，不重新执行安装。DMG 在原 invocation 内预留最多四分之一预算用于清理，取消/超时/重启后的恢复只使用原冻结 Cleanup 和递增日志序号；清理资源不改写未知安装事实，原活动未证明结束时仍保留排他。内含 PKG 必须通过原生签名与 Gatekeeper；未签名包被拒绝。

WinGet/Brew 使用签名任务中的完整源绑定、文档摘要和物料集合；Offer 阶段只读取并缓存，Start 后才准备原生源并执行。WinGet 从实际 REST source 读取并核实固定发布文档，生成本次私有 singleton manifest 交给原生 `--manifest` 安装；包 ID、版本和安装器 SHA256 来自该固定文件，scope/架构由任务确定，不在线重新解析或写入全局源。设备须已允许 LocalManifestFiles，Agent 不修改该策略；Brew 仅在原登录用户 helper 下消费固定 parentless Git commit，使用完整 tap/package 名、固定 bottle/cask 缓存，并关闭自动更新、隐式依赖与源码构建。原生调用不接受尾随参数覆盖上述选择，仍允许协议批准的 RSS_PARAM 环境输入。版本与安装效果由独立检测核实，退出零不视为已安装；重启沿用当前格式的原材料与 journal，不重新下载安装。

管理员在当前格式部署的 `software_read_grants` 中显式登记 Brew 只读凭据，例如 `{"credential":{"source":"实际 target","reference":"只读凭据引用"},"origin":"https://软件源主机"}`。source/reference 必须与签名任务一致，origin 仅允许 HTTPS origin；凭据不写入 JSON 或源 URL。先完成首次安装初始化，停止系统服务，再由服务 OS 主体运行固定服务镜像 `--config <受保护配置> --import-software-credential <target>`，通过 stdin 提供 32–512 字节 ASCII 可见只读 token（允许一个结尾换行）；该入口只向现有 OS 凭据命名空间追加，不覆盖、自动获取或扩大权限。macOS 使用 root 服务身份，Windows 使用原服务账户；重新启动后才宣告对应凭据可用。WinGet 公共源不需要读 token。软件源使用独立的无客户端证书 HTTPS 传输，仅信任部署 CA；设备 mTLS 身份不会发送给软件源。

原生源 Prepare/Cleanup 与安装共用原 attempt、manager 排他和预算。Brew 仅在 tap/URI/commit/文档完全匹配时借用已有源，借用资源不会删除；本次新建资源记录所有权后才能创建；Brew checkout 在 attempt 私有目录完成并校验，再以禁止覆盖的原子重命名发布到 tap 路径，部分 checkout 随原 attempt 清理。attempt 与 tap 须位于同一文件系统，不支持原子发布时明确失败，不退回直接写 tap。恢复只清理本次资源。WinGet 只创建本次私有 manifest。原活动结束证明绑定原内核启动世代与 runner 原进程组/Windows Job；仍在运行、无查询权限或共享源内容变化时保留证据与排他。资源清理成功不改写未知安装事实，不重置既有包管理器配置。新部署使用 deployment version 4；不提供旧软件 Ring、旧部署格式或旧库兼容入口。当前 macOS 测试覆盖真实 TLS/SQLite/Git 与固定 fixture manager；不代表 Windows 原生 WinGet 或真实软件安装验收通过。

验收入口可加 `--dmg-app <已公证 app>`，将固定候选装入只读镜像，经真实 HTTPS、系统服务、SQLite 与认证 IPC 验证安装及精确卸载；测试不修改签名信任或 quarantine。

## 显式开发刷新

源码构建不会更新已安装服务。先将新服务、桌面及 helper 候选暂存到管理员保护的位置，完成签名/摘要和当前格式配置核验，再刷新已核对归属的注册。不能把用户可写 worktree 产物直接加入生产 pins。

macOS 系统操作由管理员运行，用户 helper 操作必须在原用户 GUI 登录中运行：

```sh
python3 scripts/service/execution-macos.py refresh --scope system --binary <原受保护产物> --config <原当前格式配置> --candidate-binary <新受保护产物> --candidate-config <新当前格式配置>
```

Windows 使用同一 owner：`execution-windows.ps1 -Action Refresh -Scope System -Binary <原产物> -Config <原配置> -CandidateBinary <新产物> -CandidateConfig <新配置>`。用户 helper 不提供 refresh；由管理员发布配置后，在原登录中 remove/install。

系统刷新由同一 Rust owner 停止已核对归属的旧 helper；刷新后由原登录启动桌面注册新 helper；保留原 helper 工作目录，不代替用户会话操作。刷新原子发布当前注册引用的配置和同归属默认 pin，注册继续引用原配置路径，以免桌面留在旧 pins。Windows 生产刷新使用默认配置；自定义隔离部署由其安装 owner 处理。Windows 系统安装与持久状态核验由一次有界 LocalSystem SCM worker 运行固定候选，核验实际运行结果后移除本次 worker；管理员账号不代替 LocalSystem 读取 DPAPI 凭据。

停机前由当前与新候选 Rust 服务分别只读核验 identity-binding、execution-initialized、受保护凭据的存储位置、通信注册及当前 journal authority/schema；不取得 writer、创建状态或执行恢复。核验失败保持原进程运行；新候选必须能读取原 OS 保护凭据。ad-hoc 映像变化若不满足旧 Keychain 项的代码身份要求，刷新明确拒绝，不重建凭据或放宽 ACL。刷新只替换匹配的注册候选，保持原身份、凭据、journal、材料和 helper 工作目录；不转换旧配置、不初始化或重新注册。停止、替换或重启失败时保留可检查的注册，不能解释为完成。脚本返回 registered 仅证明注册操作，随后需获准普通用户启动固定桌面并运行 `--service-probe`，核实认证连接与 readiness。若尚未注册，诊断可连接，但任务接口仍拒绝。

## 原生开发与验收

`pnpm dev --fixture` 为持续人工操作入口，复用已有 fixture，完全不安装系统服务。`pnpm check:desktop-native --visual` 使用相同装配自动验证真实 WKWebView。

真实旅程使用显式 `pnpm check:desktop-native --controlled-service`：复用本指南受控后端及安装 owner，构建固定候选、请求原生管理员授权、暂存受保护桌面并由实际登录用户启动。已有执行注册/helper 或默认 deployment 时拒绝替换；普通 `pnpm dev` 不执行此安装操作。真实验收从认证 IPC 和服务 journal 回读请求，取消、进程终止与设备效果分别取证。

旧独立状态服务已退出。实验室管理员如发现历史注册，须先核对真实程序、参数和安装归属，再使用 OS 原生管理工具停用/撤销注册；本仓不再提供旧服务安装或兼容接口。不得为迁移删除历史凭据、journal、个人数据或覆盖未知安装。

macOS arm64 的实际执行与目标 Windows 环境按需运行的验证分别记录；#2559 独立安全验收的有效范围与状态以 tracker 为准。开发机不要求补齐 Windows 编译或真实安全矩阵才能关闭已完成实现项；fixture、服务未安装或单平台结果不能作为 Windows 验证通过的证据。固定 NSIS EXE / PKG 的构建与部署由本指南的安装入口持有；服务端 grant 签发与浏览器下载由各自产品 owner 持有。

## 固定候选安全验收

在全部源码变更与构建完成后，使用 `node scripts/native-candidate.mjs freeze --service <服务产物> --backend <controlled-backend产物> --desktop <native-acceptance桌面产物> --runtime <Host运行包> --output <新的忽略目录内candidate.json>` 固定候选。
服务单独旅程可以省略 desktop/runtime。签名在冻结前完成；验收只复制原字节，不重新签名或构建。
冻结时记录源码、锁文件和构建工具链供追溯；运行验收以固定程序摘要、代码身份、运行包完整性及适用平台/架构为准。
后续文档、提交、源码或锁文件变化无需重新冻结原产物；更换程序、代码身份或运行包后重新冻结。
验收回执将候选构建来源与当前工具运行环境分开记录；工具仍须满足当前入口自身的运行要求，不与构建时工具版本强制等值。

macOS 服务入口增加 `--candidate <candidate.json>`，其它产物参数必须与候选一致。
真实桌面入口为 `pnpm check:desktop-native --controlled-service --candidate <candidate.json>`，跳过候选构建。
先证明认证查询可用，再仅改变客户端程序路径验证同用户错误映像被明确拒绝；不可用诊断不能算攻击拒绝。
macOS 私有数据遇到扩展 ACL 会拒绝，原数据保留；不会自动改权限，部署方应核查正确主体与 ACL。

Windows 11 amd64 / arm64 目标通过固定 pnpm 环境执行 `pnpm exec pwsh -File scripts/service/verify-execution-windows.ps1 -Phase <InstallSystem|VerifyUser|RefreshSystem|RemoveSystem> -Candidate <candidate.json> -Config <受保护配置> -Output <新回执目录>`。
先由既有部署 owner 准备真实注册与受控配置；系统阶段在管理员会话执行，VerifyUser 在原非提升登录会话执行。
RefreshSystem 还要求 CurrentBinary/CurrentConfig，复用安装 owner 的持久身份检查。
该入口只证明已运行的阶段；远程/同名管道、跨用户/会话、秘密作用域、旧连接撤销和真实桌面进程链仍须补齐目标环境矩阵。
开发机不运行 Windows 入口。回执明确区分成功、失败和未执行；本轮验收范围为 macOS；Windows 未验证项继续留在 #2559，工单保持开放。

### macOS 安全矩阵

继续使用本页的服务安装和原生桌面入口。构建
`cargo build --locked --release -p execution-ipc --example macos-security-probe`
后，将 probe 签名并准备另一份不同代码身份的同工具副本；二者均在冻结前完成，
分别通过 `--security-probe` 和 `--untrusted-probe` 加入现有 candidate。
只有获准 probe 被加入实验安装的客户端 pins，错误身份副本保持未获准。
生产系统服务不提供测试命令或授权旁路。

向现有 `verify-execution-macos.py` 或
`pnpm check:desktop-native --controlled-service --candidate <candidate.json>` 增加 `--security`，
执行原生非法字段/方法/版本、大小限制、重复与过期、错误代码身份、假服务、helper/worker
边界、刷新与撤销。完整桌面旅程还覆盖真实 WKWebView、manifest 固定 Codex、
Host/launcher 故障、保留后代和未知 scope。真实 Codex 链分别验证 Host/launcher
崩溃后的整组回收；保留后代与未知 scope 复用现有 launch-fence 故障入口、冻结 runtime
和隔离存储，要求存活后代、阻断恢复、保留原 fence 以及本次拥有 scope 的回收证据。
模型端仍是确定性协议样本，
在线模型能力由既有 smoke 单独验证。

管理员授权使用 `--authorization-password-file <主仓库绝对路径/.passwd>`。
文件必须是当前用户拥有、权限 600 的普通文件；不支持 symlink。
该文件由 `/.passwd` 忽略，不复制进 worktree、回执或命令参数中的秘密字段。

回执用 `journeyStatus` 表示本次旅程结果，用 `security.scenarios` 逐项记录
`passed / failed / notApplicable / notExecuted`，完整矩阵由 `security.status` 汇总。
旅程通过但矩阵未完成时顶层为 `partial`。旧 `unexecutedSecurity` 字段直接退出，不双读旧回执。
macOS 普通 PKG 包装执行保留既有保守语义：不保证逃逸后代停止，可能返回
`outcomeUnknown / capture_failed`。回执保留 `executionUncertain`；旅程仅在原尝试、
后台确认、安装器退出及独立设备效果全部相符时接纳该已知不确定性，不改写执行成功。
刷新停止前先验证原 PID、持久事实和正常用户健康查询；停止后失败比较完整配置值，
保留注册、凭据及 journal，不要求旧 PID 存活。SQLite 权限检查不得另开、关闭活跃
数据或 sidecar 文件，以免释放同进程 POSIX 锁并破坏 WAL 共享内存。

五秒 challenge 不属于当前系统 IPC，记录为有依据的不适用；连接寿命和业务 offer 有效期分别实测。
连接场景先用现有空 helper 注册的拒绝回包建立真实 NSXPC，且不消费业务调用额度。
重启场景允许 NSXPC 按原 pins 重新认证新服务，要求旧实例终止、实际新 PID 及无新增执行证据。
未安装、未就绪、超时及未连接到攻击端点不能作为身份攻击被拒绝的证据。

跨用户和登录世代在专用可重置 macOS 环境执行，保留原实验服务、backend 和固定候选。
同一脚本提供 `--login-phase prepare|cross-user|resume|confirm`、
`--deployment-config <受保护配置>`、`--login-state <checkpoint.json>`，并使用新 `--output` 目录。
prepare 要求有效期至少五分钟的 user-initiated offer，保存风险未知而被拒绝的 AI 原请求与真实 UID/ASID；
随后由操作员切换第二真实 GUI 用户执行 cross-user，或注销/重新登录原用户执行 resume。
cross-user 的拒绝先记录为待确认；回到原用户后以 confirm 和
`--login-response <第二用户回执>` 核验服务 ready、原 offer 未变及 journal 无新事实，才记通过。
resume 同样在拒绝前后核验这些前提，不把撤销、旧产物或 offer 被替换造成的拒绝当作登录边界证据。
检查点含请求关联数据，按测试主体受控交接；不含授权秘密。resume 要求 offer 仍未过期。
脚本不创建账户、不自动注销，不使用 sudo 切 UID 代替 GUI 登录。缺环境时保留未执行，
不得把这两个场景或 Windows 计为通过。

Agent 部署生成使用 `.env` 的 RSS_AGENT_CONNECTIONS_FILE（后端下载的签名文件绝对路径）、必填 RSS_AGENT_CA_FILE；桌面管理 origin/CA 继续独立配置。部署与候选格式以当前生成器为准；生成器只写新文件，不覆盖原身份材料。设备证书经原生 CNG/daemon Keychain 签名完成 mTLS；HTTPS 只信任部署时保护的 bootstrap CA，下载的设备 CA 不扩展 HTTPS 信任；任务签名验证始终保留。

任务运行验签只读取通信 SQLite 中通过已认证设备 HTTPS 通道取得的任务公钥集合，安装 `signing_keys` 不参与运行验签。首次同步失败保留原注册身份并暂停任务准入；刷新失败保留仍可验证的旧集合，未知 key ID 拒绝执行。集合绑定 service_id/tenant，拒绝回退、同版本异内容、同 ID 换钥和非法生命周期变化。完整集合与 ACK 意图持久提交并读回后才 ACK，丢失响应或重启重试同一操作。Prepared、Active 和 VerifyOnly 公钥均可验签；执行仍须取得有效 Start 许可并满足身份、期限和输入一致性检查。离线设备可使密钥轮换待完成，不阻塞后端部署、启动或租户开通。

HTTPS 认证服务端、设备证书认证设备、任务签名检查持久任务内容、Start 决定执行许可。通过 HTTPS 更新的任务信任不构成独立于该通道的批准权威，也不防御客户端完整失陷。任务信任和 PKI 各自上报接收、应用、失败或未知观察；认证 IPC 只投影脱敏状态及独立 ACK 确认。失联时观察仅代表最后一次结果；PKI 安装摘要不代表 CRL 仍新鲜。磁盘写入失败不报告已应用。

已接受卸载在原受保护恢复载体中冻结当时的签名证据，只能继续原 installation/task/attempt；旧钥退出后该证据仍可完成原卸载，但不能成为新任务信任。升级前由原候选完成旧版未决操作；不转换历史恢复载体，不清库、改版本号或重新注册绕过拒绝。当前格式内重启和重复安装保留原身份及 journal。

设备 PKI 由现有系统服务和通信 lease 持有。服务验证完整 Agent trust 集合、独立 root/issuer 链、签名 CRL、有效期和持久化防回退水位，写入既有私有通信 SQLite。只有从该存储独立读回并确认当前设备 mTLS 凭证有效才 ACK；响应丢失重放原操作。注册、续期和刷新不要求写入或信任 macOS System Keychain / Windows LocalMachine Root。HTTPS 仍只信任部署提供的 bootstrap CA，设备 CA 不扩大 HTTPS 信任；不可导出设备私钥继续由 OS Keychain/CNG 持有。现有设备、注册、证书、journal 和历史机器证书归属数据保留；历史数据不参与当前信任判断，也不触发删除外部证书。

服务每五分钟刷新 trust，并提前检查 CRL 到期；设备凭证进入 `min(20% 生命周期, 30 天)` 窗口后自动续签。恢复沿用持久化操作、CSR、密钥和注册；创建 renewal grant 的响应丢失后，可通过 bootstrap HTTPS 查询原 grant。过期、吊销或无法验证的 trust 会关闭执行准入并停止已有工作，执行日志和副作用证据保留。

管理员恢复需要停止系统服务，准备服务端为**同一注册**签发的受保护 `GrantImport` JSON（`operationId`、`grant`），通过 `rss-execution-service --installation recover --scope system --config <部署路径> --grant <JSON路径> --secret-file <私有secret路径>` 交给原 installation owner。Windows 沿既有 SCM worker 转为 LocalSystem，macOS 使用系统管理员执行；secret 文件由原私有输入规则保护，不把 secret 放入 argv。此入口不替换未决操作，也不创建新注册；成功激活只恢复新任务准入。owner 尝试重新启动原服务；核对认证 IPC readiness，并按既有私有材料处理方式清理输入 secret。此同身份恢复入口不属于首次注册配置导入。

历史部署、通信库与卸载恢复格式保持原样并被拒绝；本仓不提供历史格式转换、兼容读取或重试入口。当前格式的安装与 apply/refresh 只处理当前候选及当前部署，沿用既有候选、代码身份、持久状态与设备归属校验。拒绝旧格式不停止原 owner、不覆盖配置或 retained manifest、不转换通信库，也不重建注册、凭据或执行 journal。管理员应保留旧部署及持久材料，不通过改写版本号、删除状态或重新注册绕过拒绝。

## Windows 原生声明式采集

标准采集使用 rss-mdm 发布的原生 Resource 与独立 Collection 任务。Agent 固定消费唯一 wire，按当前实现上报原生引擎及版本；没有 PowerShell 也可以领取原生任务。新字段通过服务端 WMI/Registry 定义和资产映射发布，不在 Agent 维护字段目录。

原生采集由同一受保护服务镜像的固定内部 worker 执行。WMI 只读本机已有类的声明属性；Registry 只读明确 hive/SID 和 32/64 位视图，不展开 EXPAND_SZ，不加载离线 hive，不执行卸载命令。CPU、网络、电池和软件使用协议规定的固定 collector。软件覆盖所有既存用户，来源缺失、未加载 hive、用户集合变化及非完整包注册均不能作为完整设备清单。

查询结果与传输完成分别判断。完整空列表可以替换旧清单，缺失、拒绝、超时及截断不伪装完整；完整查询不因另一个查询失败而丢失。完整结果固定持久化后再分块传输，丢失 ACK 或重启只补传原字节，不重新采集。worker 中断且没有完整结果时保留失败/未知证据。固定候选升级前须完成旧版未决操作；旧任务包络、签名或持久格式明确拒绝，不提供兼容解码，也不能清 journal 或重注册绕过。

旧标准 PowerShell 采集资源与导出器已退役；企业显式发布和授权的 Script 执行仍可使用，原生失败不会自动转成脚本。

本次按 #2739 冻结契约迁移，不承诺与旧脚本字段完全等价：身份来源、磁盘/卷身份及名称、异构 CPU 型号和电池汇总以新契约为准；电池采用可验证能量加权，容量不可靠时报告缺失或无效。旧 edition/architecture 映射、标准占位序列号/UUID 过滤和独立 provisioning 字段的缺口由服务端 owner 跟踪，Agent 不改写通用 WMI 原始值补偿。具体差异和后续项留在 #2740 交付记录。

开发机使用本仓 Rust 纯逻辑、SQLite/协议接缝及本地 CI，不再需要 COLLECTION_PWSH 或 PowerShell 5.1 兼容会话。纯逻辑 fixture 不证明 Windows 原生读取；Windows 编译、运行、LocalSystem、多用户、x64/ARM64 和性能必须以目标环境证据另行声明。

## macOS 原生采集

macOS 硬件、Applications、PKG receipts 和 Homebrew 使用同一 Collection 任务、journal 与补传链。
设备读取由 System broker 执行，用户 Applications 与 Brew 必须经过当前登录用户的认证 helper；
会话改变后原授权不可继续使用，不以 root 或离线账号切换兜底。应用存在、PKG 收据和 Brew 记录分别保存，
单个根或 prefix 的结果不能解释为设备全量软件快照；用户/prefix 的资产范围投影由服务端提供。
Applications 只枚举批准物理根内的应用目录，不跟随目录别名（包括 Safari 的 Cryptex 链接）；链接目标不作为该根已采集的应用。

Brew v2 只读取批准 prefix 内 Cellar/Caskroom 的安装记录与可识别 JSON，不调用 brew、Ruby 或 shell，
不依赖 `execution.managers` 中的 Brew 配置；用户可写的安装元数据按不可信数据处理。
版本与 tap 来自安装记录，不从当前仓库版本或默认 tap 推断；不能解析的历史格式、链接越界、权限不足、
扫描期间变化或预算耗尽均保留非完整状态，不能当作完整空清单。不同用户、prefix、tap、包类型与安装版本保持独立身份。
旧 Brew v1 引擎和蓝图不再支持，须先结算旧任务、保留 journal，再与服务端协调升级；不保留命令执行回退。

原生查询、清理和诊断共享任务预算。完整传输不等于完整采集，进程退出也不等于所有工作已收尾。
固定 worker 的本地收尾控制消息只记录执行事实；服务端结果仍直接使用唯一 wire DTO。
升级前完成旧版未决操作；已固定结果继续补传，未得到完整结果的执行保持未知，不能通过重启重新采集。

## 受管安装与互注册

互注册由管理员的两条 Policy 分别控制：`ensure_agent_installed` 部署 Agent，`request_mdm_enrollment` 发起原生 MDM 注册。未配置或停用策略不会因检测到缺少通道而自动注册。设置页只有收到组织发布且当前可调用的自助注册策略时，才提供人工申请入口；组织与地址由策略确定。系统确认仍需用户完成，打开界面不表示注册成功。

Windows 构建可显式选择 `--format windows_exe|windows_msi|windows_msix`，默认仍为独立安装的 NSIS；macOS 使用 `macos_pkg`。MSI 使用 WiX 的 v4 XML 格式与 `wix build`，可用 `RSS_WIX` 指定工具。MSIX 使用 Windows SDK `makeappx`（可由 `RSS_MAKEAPPX` 指定），必须配置格式需要的 `RSS_WINDOWS_PUBLISHER`；只有显式 `--release` 才要求匹配的 `RSS_WINDOWS_SIGNING_THUMBPRINT`。默认包不强制签名，实际安装仍由操作系统与企业策略判断。各格式独立发布，不自动替换或回退格式。MSIX 的机器服务由 Package Manager 管理；机器配置、密钥和 journal 位于原有 ProgramData 目录，卸载包不等于退管。

受管激活读取系统安装的专用短期引导证书，通过原生密钥句柄完成 mTLS，再使用独立本地日常密钥完成 Agent 激活。包和证书可以先后到达；私钥不可访问时等待/报错，不导出 MDM 私钥，也不自动放宽 ACL。没有交互登录时仍可运行机器注册；未配置桌面用户的安装保留空 IPC 用户集合，后续通过既有安装 profile/刷新流程配置用户访问。

设置页和 Inventory 使用同一原生注册观察。无法读取或无法确认组织归属时显示 Unknown；只有明确无 MDM 注册才报告未注册。已有原生通道但缺少可信组织入口时，不按主机名或序列号推定归属。完整 Windows/macOS 包安装、SCEP 私钥访问及原生签入由 #2803 记录真机证据。

## Windows 恢复凭据

系统服务消费签名的凭据策略任务，以稳定卷标识/protector ID 采集和轮换 BitLocker 恢复密码，
或按明确的本机管理员 SID 重置 RSS 管理的密码。首次接管是重置，不能读取未知旧密码。
Microsoft LAPS 的 CSP、组策略、原始配置或旧版 AdmPwd 策略存在时拒绝账户操作；
其他密码管理软件须通过 `HKLM\SOFTWARE\RSS\PasswordManagementOwner` 标记排他所有权。

受保护材料目录下的 `credentials` 子目录保存服务账户 DPAPI 保护的候选、原操作身份及密文补传状态。
必须连同原服务账户和设备保护环境保留；不要手工删除未决记录或以新目录重试。
新材料经服务端持久托管并由本机核实后，才删除明确被替换的 BitLocker protector。
进程中断、取消或网络失败不表示 OS 已回滚；未决操作由服务端发起绑定原 task/attempt 的核实任务恢复。
普通任务结果仅显示封闭状态，不提供密码、原生命令文本或密文下载入口。

本机测试覆盖纯恢复逻辑、加密协议和 SQLite/通信恢复。Windows 原生编译、LocalSystem DPAPI/ACL、
实际多卷轮换与账户改密、断电及真实恢复效果由 #2800 在 Windows 环境验证。

注册配置交换按当前租户、平台、架构和安装格式选择最高发布版本，并在发送前固定 releaseId。同版本使用稳定 releaseId 排序，不解释为发布时间；未知结果与重启继续原操作，不重新选择。该关联不改变本机实际安装版本，也不触发自动升级。

### 周期遥测与本地诊断

服务为新注册声明 telemetry 能力；已注册设备不会在本地擅自改写注册能力。
签名配置必须通过任务信任确认、租户/设备/注册世代绑定和短租约验证。
周期采样与命令 journal 分离，复用受控解释器及原生 collection worker；
配置到期停止采样，离线仅发送保留期内已冻结的数据，不补跑错过的采样点。
SQLite spool 独立限制为 64 MiB / 4096 次观测，按原始批次查询或重传。
本地状态的 `telemetry` 包含排队量、按原因汇总的缺口及最近持久接收回执状态；
`pending` 回执仅表示服务端持久接收，不表示分析存储已经可查询。
网络成本未知或昂贵时暂停上传，低电量停止采样，省电模式退出加速采样。
标量累计计数器从首次观察建立本地基线；启动身份变化、计数器回退或起点未知时输出 missing，
不把开机以来的累计数当作本区间增量。Windows 服务重启保守地重新建立基线。

受信安装的服务程序可执行 `--telemetry-probe`，读取本机 CPU、内存、卷和网卡计数，
仅输出本地诊断；它不接收远程参数，也不上传为已授权遥测。
原生性能采集消费服务端发布的 Windows/macOS performance V1 recipe，经同一签名配置与受限 collection worker 执行 CPU、内存、文件系统和网络读取。
CPU 在单次查询内至少间隔 250 ms 读取两次；计数域变化、回退或无有效增量时报告 missing。
网络值为累计字节数，文件系统容量按稳定卷身份分别报告，不将 APFS 共享容器容量相加。
逐接口历史 Counter 使用签名的 `counterNormalization: "local_cumulative_v1"`，要求注册能力
`telemetry.counter.local.v1`。Agent 按稳定接口键保存基线和起点，上传相对基线的累计值与逐接口区间；
首次出现、接口重现、计数回退或启动身份变化先输出 missing，不用有效零值代替预热。
同一字段包含多个接口时，任一接口预热使该次字段整体 missing，context 保留接口身份与起点。
基线与样本同事务落盘，同一启动身份下进程重启恢复已有基线；原始 UInt64 转换为 Integer 超范围时明确失败。
macOS 优先使用卷 UUID；不提供 UUID 的文件系统使用启动、原生 fsid 和挂载来源限定身份，仅保证当前挂载实例内稳定。
Windows CPU 当前仅接纳单 processor group，多组系统报告 unsupported，不以局部计数冒充整机 CPU。

缺口报告按产生时的注册身份和代际持久保存，冻结后以原报告 ID 和正文恢复回执或重传；
配置撤销不阻止已有诊断上报，重新注册不会把旧报告改绑到新注册。报告具有固定七天窗口，
收到持久回执后清除对应待传记录；没有可信注册来源的本地历史统计不能转换为远程报告。
服务端按设备权限提供缺口查询；这些设备自报诊断不证明连续覆盖率，也不替代有效样本或采样租约。
此实现与本仓 SQLite/TLS fixture 验证不代替实际 MDM 部署验收；Windows 原生路径需在目标环境另验。

远端输出 schema 只接纳有界的结构、类型、必填、枚举和数值/长度约束；
不接纳引用、正则、组合分支或自定义关键字。复杂度或本地输出验证预算超限时拒绝采集结果并记录缺口。


## 连接配置发布、更新与离线恢复

仅支持当前格式的全新安装；旧部署、旧持久格式和旧 origin 白名单明确拒绝，不自动转换或重注册。首次安装保护稳定服务身份、租户、独立验签公钥和 HTTPS CA，签名连接配置只改变 HTTPS 地址。通信 SQLite、凭据命名空间、执行 authority、内容缓存和遥测 spool 均绑定稳定身份。

管理员先在 Nginx 部署并完成真实 HTTPS 观察，再通过后端发布 `A → A+B → B`。Agent 在现有服务循环中获取配置、验签、防回滚、原子落盘并读回，随后使用首选地址上报 revision/digest 与实际入口 ACK。ACK 响应丢失重试原操作；换地址产生新 ACK 操作。仅 DNS/TCP/网络超时或断连允许在已签名候选内切换；TLS、HTTP 拒绝或限流、非法响应和签名错误不触发切换。写操作继续由原业务操作查询/重试，传输层不自动重放。

所有已知入口失联时，管理员从后端取得绑定相同服务与租户的签名连接文件，将其放在管理员保护且无符号链接的路径，调用已安装服务镜像：

```sh
<固定服务镜像> --installation import-connections --connections <受保护签名文件绝对路径>
```

该操作使用现有管理员安装操作、安装锁与独占通信租约，停止服务后验证并持久化文件，然后恢复原服务注册；失败同样尝试恢复服务。它保留原设备、密钥、registration/generation、执行日志、内容缓存和未决操作。无私钥导入，无身份重新初始化。安装程序正常刷新也保留这些状态、软件源读取授权及已安装连接配置；旧安装包的较低 revision 不覆盖本机更高 revision。

原生 Windows/Apple MDM 注册 URL 继续由各自协议材料持有，Agent 连接配置不承担原生客户端换址。软件源凭据仍按原 source/reference 与精确 HTTPS origin 独立授权；桌面网站登录入口由既有组织配置持有。

### Windows RSS 登录组件（x64）

本机设置页的“RSS 登录”只向当前已解锁的物理控制台本地账户提供首次自助关联，需有效员工登录与后台候选策略。设置验证器后提交六位动态码；创建账户时 Windows 密码只经本机受控输入交给 SYSTEM worker，不上传后台。页面的“准备完成”不代表已应用，应刷新并核对后台应用 revision。追加关联、创建其它账户、角色变更由后台 AccountWrite 授权；本地管理员身份不提供 RSS 授权。

Credential Provider 是独立 `windows-credential-provider` cdylib，与 Agent 静态链接同一离线因子 owner；当前仅交付源码与手工组件入口，安装包装配另行验收。在 Windows x64 构建环境使用 `cargo build --locked --release -p windows-credential-provider -p agent-service`，固定 Agent 和 DLL 的提交、摘要及签名后，将 DLL 放入 SYSTEM/管理员拥有、普通用户不可写的规范绝对路径。部署格式为 version 4，旧格式在候选准入阶段拒绝，不迁移；已有当前格式部署在服务切换前核对离线库与安装身份。先由系统服务的新部署初始化 `execution.material_root/device-access`；注册不能创建或替换离线库。以 LocalSystem 主体运行：

```powershell
& $Agent --device-access-component register $OfflineRoot $ServiceId $ProviderDll
& $Agent --device-access-component health
# 独立恢复入口，不加载 DLL；保留其它系统登录 Provider：
& $Agent --device-access-component disable
& $Agent --device-access-component cleanup
```

`$OfflineRoot` 是上述 device-access 子目录，`$ServiceId` 是部署配置的 `service_id`，`$ProviderDll` 是固定候选 DLL 的绝对路径。register 先检查既有离线库及 DLL，再发布固定 CLSID；health 核对注册、路径、摘要与库，不能证明 LogonUI 能加载。disable 只停用 RSS Provider；cleanup 删除其 COM/配置注册，保留 OS 账户与离线材料。正常设备 unmanage 会在移除程序前撤销匹配安装身份的组件注册。不要删除系统 Provider，也不安装 Credential Provider Filter。

离线验证先持久消费 TOTP，再向 Windows 序列化密码；Windows 密码错误仍消耗该时间步。TOTP 为六位、30 秒、前后一步，持久限速从第五次失败开始；重启不能清零失败预算，时钟回拨拒绝动态码。恢复密钥可重复使用、无期限，只替代 RSS 因子，仍需正确 Windows 密码。网络或 Agent 停机不影响已激活的离线验证。解绑只移除 RSS 集成，不删除账户、数据或角色；既有角色仅在明确授权时修改，并保护最后一个可用管理员。

目标机可在已有固定候选验收中运行 `verify-execution-windows.ps1 -Phase DeviceAccessHealth -Candidate <候选> -Config <配置> -Output <新回执目录> -ProviderDll <固定DLL> -ProviderSha256 <预先固定SHA256>`，要求 LocalSystem/x64。此阶段只记录组件健康证据。必须另在原生 LogonUI 验证首次登录/解锁、离线、错误密码后的 OTP 重放、并发消费、重启限速、时钟回拨、恢复密钥轮换、账户角色保护、卸载及故障禁用。AD/Entra/MSA、RDP、UAC 与全入口 MFA 均不在当前承诺内。开发机 macOS 不执行 Windows 构建、测试或验收。

### macOS Platform SSO 开发组件（Apple silicon / 26.4）

候选只针对 macOS 26.4 / SDK 26.4、Apple silicon；实际系统加载和回调必须在已获授权的受管测试机验证。`crates/platform-sso` 持有协议组合与恢复决策，`apps/platform-sso-extension` 是 Apple 系统入口和本地 FFI；私钥、系统令牌及注册数据库由 Apple Framework 持有。PSSO 1.0 使用共享设备密钥和 `UserSecureEnclaveKey`，保留本地密码登录，不提供密码同步、ADE/JIT、Authorization Plugin 或严格在线登录。

使用现有构建 runner 执行 `node scripts/build-platform-sso.mjs --verify-abi`，生成原生 `.appex` 并运行 C/Rust/系统签名 ABI 测试（测试进程不执行系统注册）。默认 ad-hoc 仅用于编译、链接和包结构检查，不证明系统接纳。指定 `RSS_MACOS_DEVELOPMENT_SIGNING_IDENTITY` 时，使用已获批准的开发签名材料；签名团队、profile 的 AppIdentifier、扩展 ID `com.rss.mdmagent.psso`、容器 `com.rss.mdmagent` 及 HTTPS AASA 必须匹配。现有 installer builder 嵌入同一子产物并重签容器；正式公证及安装升级验收由发布流程提供。构建输出 `systemAccepted: false` 不得改读为注册通过。

部署的可选 `platform_sso` 配置包含固定扩展 artifact、`team_id` 和配置中的 console HTTPS `origin`。仅真实安装的受保护映像通过摘要、Apple 签名与精确平台核对后才公布能力。扩展通过既有 XPC 服务调用有限注册操作，其实际进程身份不能调用 Desktop、AI 或通用执行入口。普通认证由系统直接处理，Desktop 可以退出。

Mac 设备访问页面复用员工确认及签名任务。已有账号按真实控制台会话和 GeneratedUID 关联；建号及角色变更必须由后台授权任务发起。Mac 路径不使用 Windows TOTP。密码只经短暂本机受控输入进入固定系统工具，Secure Token 需要合格管理员的系统交互授权；页面准备完成不代表账号任务或系统注册成功。Secure Token、volume ownership、FileVault 资格分别回读，无法证明的状态保持未知。解绑保留账号、数据及既有 Secure Token。

P6 可由 root 对固定安装的 Agent 调用：

```sh
"$Agent" --platform-sso-component inspect
"$Agent" --platform-sso-component disable
"$Agent" --platform-sso-component retire
"$Agent" --platform-sso-component cleanup
```

`disable` 持久阻止新注册；`retire` 将已记录的精确 source/version 交给现有服务恢复循环。远端注销使用原子版本条件，回执丢失可回读，不能撤销替换注册。首次注册结果未知且查询尚无 source 时，保留在途坐标，不能把单次未查到视为远端已清理；须等待已注册／已撤销或替换源等确定证据。明确拒绝且未提交的注册会结清对应在途记录。诊断中的 `registrationRecord` 仅区分在途与服务端接受记录，不表示 Apple 已保存配置。`cleanup` 只有在远端撤销与对应原生清理观察均被确认后才移除本机公共元数据，否则返回未完成并保留事实。它不模拟 LoginManager，不删除 OS 账号或私钥。Apple 配置移除及系统材料清理必须通过合法扩展上下文；网络、服务或该上下文不可用时不能报告注销完成。

正式支持证据必须包含签名嵌入、系统识别、系统发起的设备/用户注册回调、已有账号与受控建号、注册/认证/修复/解绑、锁屏及会话切换、旧代/重放/取消/中断/回执丢失/换钥/撤销，以及 Desktop 或 Agent 停止时的本地登录。构建与协议测试不能替代这些证据；缺受管测试机、有效签名、配置或域关联时保留真实加载/回调阻塞。来源：[Apple 注册设备和用户](https://developer.apple.com/documentation/authenticationservices/registering-devices-and-users)，SDK 26.4 AuthenticationServices 头文件。
