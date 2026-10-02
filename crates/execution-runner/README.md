# 平台执行机制

生产组合根是 `apps/agent-service`，二进制为 `rss-execution-service`。`NativeRunner` 只拥有实际进程与材料租约；通信客户端、设备凭据、IPC 和唯一执行 journal 由系统服务装配。桌面和 AI 通过同一认证 IPC 提交后台任务标识，不提交脚本、执行计划或批准声明。

后台决定企业任务授权、来源和批准。客户端验证签名，以及租户、设备、注册世代、attempt、材料和 OS 上下文的精确绑定。Offer 只准备；取得 Start 后才登记执行意图。用户主动安装的动作确认与 macOS 自身授权仍然独立存在。客户端不维护企业 Grant 表，不签发企业批准，不用测试身份回退。

设备秘密使用 macOS Keychain 或 Windows 服务账号 DPAPI。部署 JSON 只保存服务地址、公钥和受保护引用；注册重试复用原秘密和操作标识。系统服务独占通信存储与执行 SQLite；用户 helper 仅执行原登录会话中的物理调用，不保存第二份业务账本。

当前格式与认证部署方式由[本机服务操作](../../docs/guides/local-service-lab.md)持有。旧格式明确拒绝，旧库和未决记录原样保留；没有自动迁移、清库、重新注册或新建 journal 的恢复回退。

软件输入直接使用 V5 有序步骤、独立检测及 Bundle 成员长度/摘要。固定 MSI、WinGet manifest、PKG、Homebrew formula 和 Bundle 材料，不隐式选默认源或 latest。每个步骤开始前先提交 checkpoint，退出、检测和静止分别记录；同一 attempt 使用累计时间和输出预算。Start 只限制首次启动，后续步骤仍受原任务预算、取消及原 OS 会话约束。

恢复先读取同一 journal 并查询原 helper。只有明确完成的步骤边界可继续；未完成、缺证据或内容不匹配的步骤保持 Unknown，不重跑、不跳过。根进程退出事实不因整体静止未知而丢失，退出零不等于安装成功。

包管理器保留自身协调机制，Agent 通过同一 journal 的持久占用串行化冲突任务。不承诺排除管理员或其它非协作安装程序。macOS 进程组不能证明逃逸后代已经结束；检测变化或原安装活动无法确认结束时保留 Unknown。无法独立确认的已有安装来源或版本顺序不能被推断成组织所有权或允许降级。

`OsIdentity` 表示所选 OS 账号权限，不是文件/网络沙箱。解释器、材料、参数和环境均固定在输入摘要中；受保护路径和真实会话在执行前复核。材料注册有界且不可替换，磁盘材料有明确容量上限；未决占用不能通过换数据库绕过。

部署与卸载见[本机服务操作](../../docs/guides/local-service-lab.md)。平台验证结论以对应 PR 的实际运行记录为准；Windows 编译不能代表 Windows 实机安装通过。

实现参考：固定 Agent V4 `rss-mdm@025ebc5ae676763fdeb113ff4ec8f1bb374284c9` 的 `crates/agent-wire/src/tasks.rs`；zip-rs `src/read.rs@771dfc534d2614158af5497ea3dff4d4208d7db1`；Homebrew `Library/Homebrew/lock_file.rb`、`formula_installer.rb@b2cfc03346d482f79886de108fee5dc49a6efc10`；macOS SDK 的 `NSXPCConnection.h`、`audit.h`、`mach_time.h`；Windows `MsiGetProductInfoExW`、命名管道令牌和 Job API。
