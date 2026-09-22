# 双平台安全服务实验室操作

范围见[架构](../architecture/local-service.md)。本指南是验收入口，不是通过证明。需要可重置的 macOS 26.4 arm64 / Windows 11 x64。当前开发机缺少 MinGW C 编译器，完整 Windows 桌面构建及真实服务安全矩阵需对应环境补齐。

## 构建与数据

使用固定 Node 24.14.1、pnpm 11.4.0 和 Rust 工具链，先安装冻结依赖。从已提交源码执行：

    pnpm build:ai-access
    pnpm desktop:build
    cargo build --release --locked -p local-service

桌面构建选择 macOS app / Windows NSIS 配置，携带当前平台运行包；Windows 同时产出 .local-ci-runs/windows-lab-desktop，管理员安装只消费这个带摘要清单的展开目录。Windows 从 pnpm 环境运行脚本，安装 C/C++ 与 WebView2 构建先决条件。候选包不代表完成发布签名或平台验收。

AI SQLite schema 5 不迁移旧库。使用全新的实验室 OS 用户/应用数据目录；旧数据保留，旧格式打开应明确失败。不得删除原账户数据来通过测试。

## 管理员安装

macOS：

    sudo python3 scripts/service/install-macos.py --desktop 'target/release/bundle/macos/RSS MDM Agent.app' --service target/release/rss-local-service --allow-user <普通用户UID>

脚本创建非登录服务账号、固定 app/服务与 LaunchDaemon，采用 ad-hoc cdhash 固定实验室产物，无正式发布者身份承诺。已有 app 要求操作者先处理旧实验室安装；不自动删除旧 app 或恢复旧信任清单。

macOS 固定桌面安装于 `/Library/Application Support/RSS MDM Agent/desktop/RSS MDM Agent.app`；普通用户从该位置打开。其祖先目录均须 root 拥有且不可被组或其它用户写入，不使用通常带 admin 组写权限的 `/Applications`。

Windows 管理员 Windows PowerShell 5.1（Desktop edition，使用原子创建目录 ACL 的 .NET Framework API）：

    .\scripts\service\install-windows.ps1 -DesktopDirectory '.\.local-ci-runs\windows-lab-desktop' -ServiceExecutable '.\target\release\rss-local-service.exe' -AllowedUserSid '<普通用户SID>'

脚本注册虚拟服务账号，设置目录与查询 ACL。固定 SHA-256 来自管理员安装产物；不以未验证的签名链代替固定映像身份。

获准普通用户打开桌面设置中的“本机安全服务”，应显示已连接/statusOnly。运行 node scripts/verify-local-service.mjs <安装后的桌面可执行文件> <target/release/rss-untrusted-service-probe 或 .exe> 将同一安装/构建的正向查询、独立进程负例与再次正向查询写入同一回执；任一步失败都不能标记通过，其它攻击场景仍单独验收。

## 撤销与卸载

    sudo python3 scripts/service/manage-macos.py --revoke-user <UID>
    sudo python3 scripts/service/manage-macos.py --uninstall
    .\scripts\service\manage-windows.ps1 -RevokeSid '<SID>'
    .\scripts\service\manage-windows.ps1 -Uninstall

撤销后旧连接和新查询都应失败。卸载只移除服务注册，保留安装文件和 AI 数据。不回滚业务效果，不宣称 provider 密钥已撤销。

## 必须补齐的平台验收

每个场景记录 OS/架构、源码 SHA、lock、固定产物摘要、OS 主体、命令、原始响应和 handler 是否获准进入。账号、机器绝对路径与秘密保留在忽略的本地回执，不提交仓库。

| 场景 | 实施入口/检查 | 预期 |
| --- | --- | --- |
| 双向身份 | 已安装桌面查询；假服务端；替换客户端/服务文件 | 仅正确双方可查询 |
| 独立恶意进程 | 正向查询已通过后运行 rss-untrusted-service-probe；另一个普通用户访问 | 未获准，不能把服务未安装当拒绝证明 |
| worker 越权 | worker 对同一端点发起查询 | 缺少桌面进程身份，拒绝 |
| 重放与到期 | 核心套件先通过；真实 IPC 重发 challenge、重复请求、跨连接、超过五秒 | handler 不执行或同连接最多一次 |
| 撤销/重启 | 保持旧连接并运行维护命令或重启，再提交旧请求 | 旧连接失效，撤销主体不能重连 |
| Windows 边界 | 远程 pipe、错误 SID/session、同名端点、弱 ACL/reparse | 拒绝，不降级为匿名或 TCP |
| macOS 边界 | 错 cdhash、同 UID 其它映像、错误 audit session、替换路径 | 拒绝，不只依赖 UID |
| 桌面/Codex | 普通用户窗口、状态查询、固定 Codex 0.155.0 对话、取消/关闭 | UI/进程回执分开，保留 S1 标识 |
| OS 生命周期 | 杀 Host/launcher、保留后代、登记失败、关闭超时、旧 scope 不明 | 阻断不确定恢复；终止有范围为空证据 |

cargo test -p local-service、Host/desktop 测试及交叉编译只是前置证据。真实矩阵全部完成前，#2462 不关闭，也不宣称双平台安全验收通过。
