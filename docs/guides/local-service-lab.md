# 唯一执行服务的部署与实验室验收

生产组合根为 agent-service，桌面状态和业务任务使用同一认证 IPC。安全机制由 installation-security 持有；用户 helper 只执行原登录会话中的物理操作。

开发机按[验证规则](../rules/verification-scope.md#开发机平台验证与实现项关闭)只运行当前宿主平台的必要验证，不执行 Windows 编译、测试、打包、安装或真机验收，也不补齐 Windows 环境。下文 Windows 部署和诊断操作仅供目标 Windows 环境按需使用；未执行时记录为未验证，不阻塞已完成实现项的关闭。

## 固定安装包与首次注册

`pnpm build:installer --organization /absolute/public-organization.json` 在当前原生平台构建 macOS arm64 PKG 或 Windows amd64 / arm64 的 perMachine NSIS EXE，不套用 MSI。macOS 构建命令通过 `python3 scripts/build-run.py -- pnpm build:installer ...` 复用本地 Cargo 槽位；Windows 使用 `pnpm exec node scripts/build-installers.mjs ...`。Windows 构建进程、Rust 工具链和 Node 必须为目标原生架构，不接受仿真进程或混合 PE。目标设备须预装 machine-wide WebView2；安装器缺失时明确拒绝。当前机器有匹配架构、受保护的 PowerShell 7 时才登记该解释器与相关能力。

组织输入仅含 `origin`（规范 HTTPS origin）、`tenant`（非零 UUID）、`label`、`signing_keys`（key ID → base64url Ed25519 公钥）与必填 `ca_pem`（HTTPS bootstrap CA）。设备 CA/CRL 从该 HTTPS origin 的公开 trust API 获取；HTTPS bootstrap CA 与程序签名证书独立；输入不允许 grant、secret、账号目录或私钥。包内固定相同组织上下文的生产服务、桌面、Node、AI Host、worker 和资源。产物及其 `package-manifest.json` 位于忽略的 `.local-ci-runs/installers/`，记录源码、锁文件、平台、候选及最终包摘要。

默认 macOS 为 ad-hoc 开发签名，Windows 为明确标记的开发包。正式包增加 `--release`：macOS 设置 `RSS_MACOS_APP_SIGNING_IDENTITY`、`RSS_MACOS_INSTALLER_SIGNING_IDENTITY` 和已存在的 `RSS_MACOS_NOTARY_PROFILE` Keychain 引用；Windows 设置 `RSS_WINDOWS_SIGNING_THUMBPRINT`，可选 `RSS_SIGNTOOL` 完整路径。构建不会接收证书密码；正式 macOS 包完成 notarization / stapling，正式 Windows 包与原生资源进行 Authenticode 签名。缺少正式签名配置会失败。

每次构建安装到独立版本目录，升级先核对完整候选、原生架构、签名及持久状态，再停旧服务和原子发布当前格式部署配置。失败保留可核实的注册与状态；候选活动未确认停止时不切回旧配置。ad-hoc 服务更换代码身份后无法读取旧 Keychain 项，会在停机前拒绝升级，不重建凭据或放宽 ACL。旧格式配置不会迁移或兼容读取。

普通安装不创建 Enrollment、设备密钥或 journal；桌面通过认证 IPC 发起机器注册。管理员也可导入独立的安装材料：公开 envelope schema 由实际 Rust decoder 生成，位于 `apps/agent-service/schema/install-bootstrap-v1.schema.json`，GrantReceipt 定义直接从固定协议包派生，公开 schema 可独立解析；服务端保持唯一协议来源。envelope 的 `origin`、`tenant`、`target`、`installation_artifact` 必须与已安装候选完全相同；后者取 `package-manifest.json` 的 `candidateSha256`。材料还含原始 `operationId` 与服务端 GrantReceipt，secret 单独置于 root / LocalSystem 私有文件或标准输入中，不进入包、argv、日志或 URL。

```sh
# 管理员准备受保护的 envelope；secret-file 及其直接父目录都须由系统账号私有持有。
# macOS：root 所有，父目录 0700、文件 0600；Windows：仅 SYSTEM / Administrators 可访问。
<受保护的服务程序> --installation import --envelope /absolute/bootstrap.json --secret-file /absolute/private-secret
# 服务已停止时，也可通过受控私有 stdin 初始化。
<受保护的服务程序> --initialize /absolute/bootstrap.json < /absolute/private-secret
```

导入先验证组织、目标原生架构、候选摘要、有效期和原操作，之后停止匹配服务、提交原操作并恢复服务。网络结果不确定时保留同一操作恢复，不自动重新注册。完成后由材料 owner 删除本次私有输入；安装器不删除共享凭据。安装成功与认证 readiness 分开核实，使用获准的普通用户运行已安装桌面的 `--service-probe`。

初装从实际互动登录发现主体及 helper 路径；无人登录的静默初装必须提供管理员保护的 profile（`subjects` 与 `helper_work_roots`）。Windows EXE 接受 `/S /PROFILE=绝对路径`；macOS 可先固定 profile 于 `/Library/Application Support/RSS MDM Agent/install-profile.json` 再运行 `installer -pkg ... -target /`。桌面启动请求 Rust owner 注册当前配置用户的 helper，AI Host 随该桌面运行。

卸载只移除匹配注册与程序；设备身份、Keychain / DPAPI 引用、通信库、journal、材料及用户数据保留。Windows 使用系统卸载入口或 `uninstall.exe /S`；macOS 管理员使用 `<受保护的服务程序> --installation uninstall --binary <当前配置的服务程序>`。重装核对 retained 当前格式上下文并复用安装 ID，原程序已移除时由新候选只读检查 retained 密钥与日志。没有隐式清库、换库或共享凭据删除入口。程序先从 versions 退役到精确版本的 retired 目录；删除失败可用同一版本入口重试，不清理其它版本。

Windows 安装操作返回 UUID 和不含输入内容的阶段回执。等待超时表示结果未知，worker 可能仍在运行；使用 `--installation operation-status --operation <UUID>` 核实，确认进程退出后回收该 worker。回执保留失败阶段、闭合错误、可得的原生退出码和补偿结果；超时后不要据命令退出自行删除正在使用的程序。

## 后台授权执行服务

构建 `cargo build --release -p agent-service --bin rss-execution-service`。生产服务与 user helper 使用同一二进制；原 execution-runner 二进制装配已删除。系统服务独占凭据和执行 journal，helper 不初始化业务数据库。

由管理员准备受保护的部署 JSON：macOS 默认 `/Library/Application Support/RSS MDM Agent/execution.json`，Windows 为 ProgramData 下同一产品目录的 `execution.json`。格式由 `apps/agent-service/src/deployment.rs` 持有，使用当前格式显式配置 version、ipc_version、HTTPS origin、tenant、signing_keys、installation_id、installation_artifact、ca_file（受保护的 HTTPS bootstrap CA）、registration_clients 桌面程序/用户 pin、state_root、service 产物 pin、clients OS 主体/程序 pin、execution 的 work_root/material_root/interpreters/managers/processes，以及每个已登记用户的 helper_work_roots。解释器和包管理器使用完整路径和固定摘要，不使用 PATH 查找。material_root 与 state_root 分离。

桌面使用根 `.env` 的可选 `RSS_MDM_CA_FILE`；后台 Agent 必须配置独立的 `RSS_AGENT_CA_FILE`，指向部署方明确提供的单个 HTTPS bootstrap CA 公共证书。桌面构建嵌入其公共证书，Agent 的 `execution.json.ca_file` 由 Agent 输入生成。生成新安装配置时先准备包含程序 pins、工作目录、注册身份等完整字段的模板，然后执行：

```sh
node scripts/agent-organization.mjs --config /private/execution-template.json --output /private/execution.json --ca-output /private/organization-ca.pem
```

`--ca-output` 是 CA 在设备上的最终绝对路径，生成器写入同一公共证书并记录该路径；Agent 必须提供该参数。输出配置与 CA 为 0644（公开信息），安装后的父目录须允许用户 helper 遍历、文件由管理员持有且普通用户不可写；凭据和状态目录保持原有保护。输出文件必须不存在，不覆盖既有部署；保持模板的其它字段，仅替换组织地址、租户和 CA。按原安装要求将配置与证书置于管理员保护的最终路径，再进行首次注册。部署不同设备时使用目标设备对应的绝对路径。现有设备不要通过生成新身份或重新注册来更新 CA；沿用原身份，按受控刷新流程发布新配置。更新 CA 后桌面重新构建、Agent 重新加载；HTTPS bootstrap 配置不改系统信任；设备 CA 的机器安装由 PKI owner 驱动。CA 私钥不分发，任务签名公钥与组织 HTTPS CA 分开管理。

首次注册可由固定桌面在本机服务设置中发起授权请求，再在组织网页核对请求编号及公钥指纹并批准。导入网页生成的材料时，使用上文「固定安装包与首次注册」的 Bootstrap envelope、实际 decoder schema 和私有 secret-file 导入入口；材料必须绑定当前组织、原生架构和已安装候选摘要。采用受控私有 stdin 的 `--initialize` 路径时，先停止原服务 owner，再由系统账号运行；秘密不得放入 argv、JSON、日志或 shell 历史。注册重试使用同一配置、原 grant/CSR/签发/激活操作和 OS 密钥。人工换证先停止原 owner，再运行 `--renew-certificate`；保持原 Registration、存储位置和 journal。初始化遇到已有不支持的数据库或缺失的原注册状态时拒绝，保留原文件；不能换目录、清库或重新注册来绕过未决任务。

macOS 系统注册：`python3 scripts/service/execution-macos.py install --scope system --binary <root 拥有的绝对产物路径> --config <受保护配置>`，需要管理员执行。用户在自己的 GUI 登录中运行同一安装脚本，改为 `--scope user`；程序参数固定为 `--user-helper`，只注册当前实际 OS 会话。`--config` 可省略以使用默认部署路径。状态查询用 `rss-execution-service --config <配置> --query`，客户端和服务必须互相匹配真实 OS 身份与程序 pin。

Windows 使用 PowerShell 7 运行 `scripts/service/execution-windows.ps1 -Action Install -Scope System -Binary <绝对 exe 路径> -Config <受保护配置>`，SCM 使用 LocalSystem。用户 helper 用 `-Scope User`，调度任务以实际登录账号启动。跨会话不继承系统进程句柄，使用认证命名管道派发。

卸载将 install/Install 改为 remove/Remove，并提供原始精确二进制和配置路径。install 拒绝已有注册；卸载核对归属，只移除注册，不删除凭据、journal、材料或审计。生产桌面读取默认受保护部署 pin；自定义配置用于显式命令行部署和隔离验收，不作为桌面失败后的回退。

自助目录的准入主体由后端返回：设备准入任务无需企业登录，企业用户准入任务需要当前有效企业会话。企业登录不授予本机管理员权限；系统账户与本机登录用户的执行身份另行核验。退出或切换企业账号后，目录、参数表单和旧 AI 入口失效；已接受的设备请求保持原 UUID、OS 会话、AI 来源和人工确认，可继续查询或恢复，企业用户请求不能沿用已撤销的企业会话。OS 会话变化仍禁止向另一会话派发。

本地执行、桌面/系统 IPC、执行 journal 和通信库按各自代码持有的格式版本拒绝旧数据；远程 wire 由固定 backend artifact 持有。远程旧协议和旧通信库明确拒绝，保留原文件，不迁移、自动换库或重新注册。测试执行器仅用于测试专用装配。按实际运行的平台分别记录编译、测试与验收结果；开发机不承担 Windows 验证，未执行的环境不记为通过。

本次合同切换仅支持新部署。旧节点先由旧服务完成任务、清理与结果补传；未决事实保留并阻止对应节点切换。旧库即使无未决任务也不提供原地 schema 迁移，保留原部署配置、身份、OS 密钥/Keychain、凭据与 journal；新环境使用独立状态目录和空库显式注册 V1。不得自动换库、清库或重注册原节点。

软件脚本检测器在退出码为 0 且完整捕获 stdout 时读取一个严格 JSON 对象：`{"kind":"absent"}` 或 `{"kind":"present","version":"固定版本"}`。其它输出、截断或无法核实的执行活动保持 Unknown；检测事实与安装进程退出分别记入同一 journal。

执行宿主机制诊断：macOS 可用 `log show --last 10m --predicate 'subsystem == "com.rss-mdm.agent.execution"'` 查看闭合的阶段/失败分类；Windows 在 Application Event Log 查看 source 为 `RSS Execution` 的事件数据（不要求自定义消息资源安装）。日志由 OS 留存，卸载不删除历史。launchd 初始化结果不确定会尝试 bootout；补偿失败保留 plist，需核对 endpoint 后重试 Remove，不直接删配置冒充回收完成。

受控 macOS 接线验收先以 release profile 构建 `agent-service` 的 `rss-execution-service` 和 `controlled-backend` example，再运行 `python3 scripts/service/verify-execution-macos.py --binary <构建产物> --backend <example产物> --output <不存在的本地回执目录>`。入口通过原生管理员授权安装隔离配置，使用真实 HTTPS、系统 Keychain、launchd IPC、系统/用户脚本和固定 PKG；只卸载本次注册，保留凭据、journal、材料和包收据供核查。已有执行服务或 helper 注册时拒绝替换。回执的失败或缺失不能算通过。

每次受控旅程只请求一次管理员授权，限于预先固定的安装、初始化、重启和清理操作；不保存密码。授权等待超过 120 秒即终止，迟到授权不能创建安装；取消和超时分别保留回执，重新开始旅程才会再次请求授权。

如明确选择本地密码文件，可为两个受控入口提供 `--authorization-password-file <绝对路径>`。文件须属于当前登录用户、仅该用户可读写（`0600`）、为普通文件且非符号链接；不得提交。密码仅通过 AppleScript 标准输入提供给原生授权，不进入 argv、日志、部署配置或回执。验收不创建或复制密码；`.passwd` 已由 Git 忽略。

材料目录最多 8 GiB / 32,768 条目，硬链接按同一文件身份计费，每个目录和文件名占一个条目。系统服务独占材料目录，并绑定设备权威及原 journal 文件身份；不匹配时拒绝打开，不覆盖归属。服务在提交之间完整读取 journal 与补传状态，只回收已安全结清且无活跃引用的精确材料；Unknown、补传未完成、未知来源和无法核实的材料保留。达到配额时拒绝新材料。管理员不得清空材料目录、删除归属文件或替换 journal 来绕过未决任务；新部署使用独立材料目录。


### 已发布 SQL 模板采集

生产配置的 `execution.interpreters` 可以登记 `osquery` profile，映像仍使用受保护的完整路径和固定 SHA-256。Agent 签入声明本次配置中的执行器，服务端只下发可执行的模板。SQL 文本取自固定摘要的资源文件，签名任务仅带模板身份、冻结参数和预算；没有临时 SQL 输入入口。参数通过共享 AST 转换成字面量，Agent 再次检查单表 SELECT、表/列/函数许可及平台。调用固定关闭扩展、事件、分布式查询、数据库和外部 flagfile，并设置一行溢出检测。

脚本和 SQL 结果继续通过任务 journal 与通信 SQLite 补传；大输出拆成 256 KiB 的固定块，总量不超过 16 MiB。只有全部块和整体摘要匹配后才提交终态结果。丢失响应重放原操作，重启不重新执行查询，也不追加客户端批准。

真实 osquery 接缝使用 `OSQUERY_TEST_BINARY=<已独立验证的官方二进制绝对路径> python3 scripts/build-run.py -- cargo test --locked -p execution-runner --test osquery -- --ignored`。macOS 必须保留官方 `.app` 完整结构以验证签名，单独复制 Mach-O 文件会破坏签名。该测试验证固定参数、字面量绑定和成功零行；不替代系统服务身份或完整平台部署验收。可复现候选为官方 5.23.1 的 `osquery-5.23.1_1.macos_arm64.tar.gz`，SHA-256 为 `5484f0b62e05a7b2fa9d6e43f038915ea2b7ce063d59bd671ede0cf8dd0552da`。

执行上下文使用本机数值 OS 版本和认证 helper 的登录世代，待确认注册与 claim 保留原上下文。生产编译器消费当前远程 V1 的 MSI、PKG、Bundle、离线 EXE、MSIX 和 DMG；WinGet/Brew 原生发布源仍不支持。EXE 仅执行完整离线布局、显式参数和明确卸载入口；退出码与重启按源任务策略解释，不自动重启。实际启动 EXE 后，当前 V1 没有可信外部安装会话完成协议，因此保留真实退出码与重启事实，同时报告 Unknown、保留离线布局与排他；Job 归零或检测命中不解除该限制。MSIX 明确区分目标用户注册与未来用户设备 provisioning，后者不改写已有用户注册；只查询既有侧载/依赖，不导入证书、改策略或终止应用。DMG 用镜像的完整长度/SHA256 确认内容身份，只读挂载并精确选择 app/内含 PKG，不计算独立 app 目录摘要；app 保留内部链接与 quarantine，经原生签名/Gatekeeper 检查再复制或原子替换。运行中应用拒绝替换/删除；普通 app 卸载直接核对声明的目标、Bundle ID、版本、上下文、签名与架构，不挂载镜像或比较镜像目录；删除使用本次尝试的短期对象快照和私有隔离，失败保留清理记录。卸载不清理用户数据。挂载、staging 和物理阶段在同一 attempt 中核算，Unknown 保留证据与排他，不重新执行安装。DMG 在原 invocation 内预留最多四分之一预算用于清理，取消/超时/重启后的恢复只使用原冻结 Cleanup 和递增日志序号；清理资源不改写未知安装事实，原活动未证明结束时仍保留排他。内含 PKG 必须通过原生签名与 Gatekeeper；未签名包被拒绝。

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
候选绑定源码、锁文件、程序摘要、代码身份与运行包完整性；改代码、改锁或换产物后重新冻结。

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

Agent 部署生成使用 `.env` 的 RSS_AGENT_ORIGIN、必填 RSS_AGENT_CA_FILE；桌面管理 origin/CA 继续独立配置。使用 Deployment 5、Candidate 2；生成器只写新文件，不覆盖原身份材料。设备证书经原生 CNG/daemon Keychain 签名完成 mTLS；HTTPS 只信任部署时保护的 bootstrap CA，下载的设备 CA 不扩展 HTTPS 信任；任务签名验证始终保留。


设备 PKI 由现有系统服务和通信 lease 持有。服务获取并验证完整 Agent trust 集合、签名 CRL、有效期和持久化防回退水位后，把 root/issuer 分别安装到机器 Root/CA 存储（macOS System Keychain 的 Admin root trust，issuer 不设独立 anchor）。只有独立回读成功、当前设备 mTLS 凭证有效时才 ACK；重试保留同一操作。原有或崩溃后无法证明归属的证书不删除；物理证书添加完成后先持久化 Owned，再配置其机器 trust，暂态设置失败可沿原归属重试。若崩溃发生在持久化 Owned 前且材料存在但设置缺失，保持拒绝，由管理员核对同一 DER 后修复机器设置，不把 presence 推断为 ownership。

服务每五分钟刷新 trust，并提前检查 CRL 到期；设备凭证进入 `min(20% 生命周期, 30 天)` 窗口后自动续签。恢复沿用持久化操作、CSR、密钥和注册；创建 renewal grant 的响应丢失后，可通过 bootstrap HTTPS 查询原 grant。过期、吊销或无法验证的 trust 会关闭执行准入并停止已有工作，执行日志和副作用证据保留。

管理员恢复需要停止系统服务，准备服务端为**同一注册**签发的受保护 `GrantImport` JSON（`operationId`、`grant`），通过 `rss-execution-service --installation recover --scope system --config <部署路径> --grant <JSON路径> --secret-file <私有secret路径>` 交给原 installation owner。Windows 沿既有 SCM worker 转为 LocalSystem，macOS 使用系统管理员执行；secret 文件由原私有输入规则保护，不把 secret 放入 argv。此入口不替换未决操作，也不创建新注册；成功激活只恢复新任务准入。owner 尝试重新启动原服务；核对认证 IPC readiness，并按既有私有材料处理方式清理输入 secret。初始 Bootstrap envelope 保持版本 1。

旧 Deployment 4 / Candidate 1 保持原样并被拒绝；本仓不提供历史格式转换、兼容读取或重试入口。当前格式的安装与 apply/refresh 只处理当前候选及当前部署，沿用既有候选、代码身份、持久状态与设备归属校验。拒绝旧格式不停止原 owner、不覆盖配置或 retained manifest、不转换通信库，也不重建注册、凭据或执行 journal。管理员应保留旧部署及持久材料，不通过改写版本号、删除状态或重新注册绕过拒绝。

## Windows 原生声明式采集

标准采集使用 rss-mdm 发布的原生 Resource 与独立 Collection 任务。Agent 固定消费唯一 wire，按当前实现上报原生引擎及版本；没有 PowerShell 也可以领取原生任务。新字段通过服务端 WMI/Registry 定义和资产映射发布，不在 Agent 维护字段目录。

原生采集由同一受保护服务镜像的固定内部 worker 执行。WMI 只读本机已有类的声明属性；Registry 只读明确 hive/SID 和 32/64 位视图，不展开 EXPAND_SZ，不加载离线 hive，不执行卸载命令。CPU、网络、电池和软件使用协议规定的固定 collector。软件覆盖所有既存用户，来源缺失、未加载 hive、用户集合变化及非完整包注册均不能作为完整设备清单。

查询结果与传输完成分别判断。完整空列表可以替换旧清单，缺失、拒绝、超时及截断不伪装完整；完整查询不因另一个查询失败而丢失。完整结果固定持久化后再分块传输，丢失 ACK 或重启只补传原字节，不重新采集。worker 中断且没有完整结果时保留失败/未知证据。固定候选升级前须完成旧版未决操作；旧任务包络、签名或持久格式明确拒绝，不提供兼容解码，也不能清 journal 或重注册绕过。

旧标准 PowerShell 采集资源与导出器已退役；企业显式发布和授权的 Script 执行仍可使用，原生失败不会自动转成脚本。

本次按 #2739 冻结契约迁移，不承诺与旧脚本字段完全等价：身份来源、磁盘/卷身份及名称、异构 CPU 型号和电池汇总以新契约为准；电池采用可验证能量加权，容量不可靠时报告缺失或无效。旧 edition/architecture 映射、标准占位序列号/UUID 过滤和独立 provisioning 字段的缺口由服务端 owner 跟踪，Agent 不改写通用 WMI 原始值补偿。具体差异和后续项留在 #2740 交付记录。

开发机使用本仓 Rust 纯逻辑、SQLite/协议接缝及本地 CI，不再需要 COLLECTION_PWSH 或 PowerShell 5.1 兼容会话。纯逻辑 fixture 不证明 Windows 原生读取；Windows 编译、运行、LocalSystem、多用户、x64/ARM64 和性能必须以目标环境证据另行声明。
