# 本地 AI Host 应用

本应用装配 AI 契约、SQLite、独立 provider worker 和 ACP–A2UI 服务。原生桌面持有测试用户选择、UI generation 和一个设备执行服务；Host 持有个人连接、产品 Session、provider 阶段及持久交付。当前验证平台为 macOS arm64。

```sh
pnpm build:ai-host
pnpm bundle:ai-host
pnpm desktop:build
```

启动配置由 [应用入口](src/) 和 [运行时 schema](../../packages/ai-contract/schema/) 持有。Native 经私有继承管道绑定可信 caller/generation，Host 不开可发现的凭据 socket，也不建立按用户服务池。信任与恢复设计见[架构](../../docs/architecture/ai-host.md)。

## 连接来源

| 来源         | Codex                                      | Claude                                             | DeepSeek           |
| ------------ | ------------------------------------------ | -------------------------------------------------- | ------------------ |
| 本机已有配置 | 直接设置官方 `CODEX_HOME`，默认 `~/.codex` | 直接设置官方 `CLAUDE_CONFIG_DIR`，默认 `~/.claude` | 不提供             |
| 自定义 API   | URL、API Key、模型                         | URL、API Key/Auth Token、模型                      | URL、API Key、模型 |

本机已有配置的登录、账号识别、凭据读取与刷新均由官方工具负责。RSS 不解析 auth.json、提取 OAuth token、访问外部工具 Keychain 或复制登录状态；模型留空时采用官方默认配置。所用 Codex app-server 不支持 CLI 命名 profile，界面不提供该选项，也不实现替代配置解析器。

用户的配置目录与 RSS 运行目录分开。RSS 不向用户目录写入 config.toml；Codex 通过运行参数限制工具、hooks、plugins 和 MCP。官方配置表递归合并，因此在创建线程前读取官方配置 API，对每个非 RSS MCP 显式禁用，并校验实际工具清单。Claude 使用 user 设置来源，同时显式关闭 hooks、plugins、skills 和任意 MCP，只保留产品允许的工具。项目设置不能覆盖产品限制。恢复只使用 RSS 自己登记的原生会话 ID，不枚举或清理用户的其它会话。

自定义 API 在 WebView 密码框填写，可粘贴、显示/隐藏，编辑时不回填。秘密经一次受限 Tauri IPC 交给 Rust，提交、取消、离开页面与切换用户时清空表单。Rust 独占应用主密钥和 AES-256-GCM 加解密（随机 96 位 nonce、128 位 tag），AAD 绑定用户、连接、provider、规范化 endpoint 和凭据类型。Host 只接收新密文并与配置原子保存；普通编辑和测试成功直接保留密文。只在 provider 激活时经私有通道解密，明文不返回 WebView。已有密文而主密钥缺失时拒绝重建。

自定义 API worker 只连接 Host 为该配置建立的随机 loopback 路由，不直接连接用户填写的地址。Host 在每次上游请求中解析并校验全部 A/AAAA 结果，将批准地址固定到实际 socket，拒绝私网、链路本地、混合结果和所有 redirect；HTTP 只允许固定 loopback 目标。路由不注入密钥，只把 worker 已携带的请求转发到该唯一目标，并随 worker 关闭。

保存只校验本地结构并写入 unverified 新修订，不启动 provider。测试已保存版本会发送真实模型探针，可能产生费用；完成且进程停止后才追加 ready 修订。失败只更新安全结果，仍可编辑、保存和重试；复测失败不撤销此前成功状态。编辑保存回到 unverified，并清除该连接的默认选择。保存、测试提交及删除分别比较当前修订，测试期间允许编辑保存，过期结果不能覆盖新配置。Codex 测试采用 ephemeral 线程，Claude 使用 persistSession:false。删除清除该连接全部修订密文与默认选择，保留元数据和会话历史。首条可用连接成为默认，后续新增不替换默认，删除默认无自动替补。

## 产品会话与交付

创建产品 Session 不启动 provider。首条输入才打开阶段，阶段固定 connectionId/configRevision；选择新连接后已接收队列按原阶段完成，下一条输入建立新阶段。每个 Session 最多一个 live worker，加应用总量上限，没有外部账号配额。原命令重试先返回旧回执；恢复失败不静默重放或换上下文。

worker 的 activation 数据通过既有私有管道传入，包含该次启动所需的配置与秘密；不写参数快照、artifact URL、公共 wire 或日志。主密钥不进入 worker。持久 launch fence 继续承担进程恢复核对，原生会话索引继续承担 RSS 自有历史定位。

历史预览只包含已完成用户输入和稳定助手文本；用户选择最近 N 轮或全部并确认。预览绑定目标配置版本、水位、命令/消息 ID 和内容哈希，不包含工具、系统指令或原始附件。设备任务始终保留冻结 actor；模型终态不等于设备业务完成。

当前无历史数据升级要求，不实现旧凭据、账号或参数快照的兼容读取、迁移和清理流程。生产执行服务与本应用凭据链隔离，见[安全服务架构](../../docs/architecture/local-service.md)。

## 验证与来源

`pnpm test:ai-host` 覆盖真实 SQLite、worker 进程、取消、阶段和交付恢复；`pnpm test:ai-acceptance` 使用固定 SDK/native 进程和本地模型协议服务。加密测试注入测试主密钥，不访问用户 Keychain。

`pnpm check:desktop-native` 从根 `pnpm dev` 启动真实产品 WebView，以固定 Codex CLI、本地协议夹具和合成凭据验证表单 → Rust 加密 → SQLite 保存 → 测试失败重试 → Host 重启 → 删除，并验证一次性工具许可和独立 Rust 动作确认。该入口使用实际系统密钥 backend，不读取个人 AI 配置；输出 `.local-ci-runs/desktop-native.json`。范围与前置条件见[桌面开发](../../docs/guides/desktop-development.md)。

`pnpm check:connection-sources` 把当前用户已有配置目录交给官方 Codex/Claude，发送最小真实模型请求。两个来源均须完成探针；目录缺失记 partial，认证或能力失败仍判失败。报告不包含账号、目录或秘密，该入口复用固定 Node/原生 helper runtime，不纳入无凭据 CI。`pnpm check:connection-sources --preflight` 只验证该入口的原生私有文件/SQLite 初始化和关闭，不访问个人配置或发送模型请求。平台窗口验收见[桌面指南](../../docs/guides/desktop-development.md)。

来源：Rust `std::os::unix::net::UnixStream::pair`；[Codex 0.155.0 config merge](https://github.com/openai/codex/blob/rust-v0.155.0/codex-rs/config/src/merge.rs) 与 [CLI profile 入口](https://github.com/openai/codex/blob/rust-v0.155.0/codex-rs/cli/src/main.rs)；Claude Agent SDK 0.3.277 `sdk.d.ts`；[Node `http.request` 自定义 `lookup`](https://nodejs.org/api/http.html#httprequestoptions-callback)；[OWASP SSRF DNS/redirect 防护](https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html)；[RustCrypto AES-GCM](https://github.com/RustCrypto/AEADs/blob/aes-gcm-v0.10.3/aes-gcm/src/lib.rs)；security-framework 3.5.1 `src/passwords.rs`、`src/random.rs`。没有复制上游认证实现。

生产执行服务采用真实 OS 对等身份，与本应用的 AI 凭据链隔离。见[架构](../../docs/architecture/local-service.md)和[实验室指南](../../docs/guides/local-service-lab.md)。

## 能力与运行范围

三个 provider 均提供普通会话与跨进程上下文续接；Host 持有 FIFO。Codex 提供 steer/fork，Claude 与 DeepSeek 提供结构化问题。取消确认仅表示已请求，不证明模型、进程或设备业务已经终止。

产品受控工具目前只准入 macOS arm64 上的 Codex；其余 provider 明确拒绝。组件协议测试不能扩大这一准入，也不证明任意同 UID 文件、网络或 IPC 隔离。

`pnpm test:ai-acceptance` 运行真实原生进程与本地模型协议服务；标准测试结果判断通过或失败。外部模型分别通过 `pnpm smoke:codex`、`pnpm smoke:claude`、`pnpm smoke:deepseek` 验证，配置见各 adapter README。没有明确配置时不搜索个人目录或自动替换为 fixture。自定义网关可达不等于验证了上游模型身份。
