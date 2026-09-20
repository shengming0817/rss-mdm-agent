# 本地 AI Host 应用

本应用装配 V5 契约、SQLite schema 4、独立 provider worker 和 ACP–A2UI 服务。原生桌面持有测试用户选择、UI generation 和一个设备执行服务；Host 持有个人连接、产品 Session、provider 阶段及持久交付。当前验证平台为 macOS arm64。

```sh
pnpm build:ai-host
pnpm bundle:ai-host
pnpm desktop:build
```

`host.json` 只包含 `version: 1`、`databasePath`、`nativeDirectory`、`workingDirectory`。Native 与 Host 通过匿名 socketpair 的继承 fd 3 通信，stdio 单独承载 Rust 执行 MCP；control frame 与 execution-origin 由 AI Runtime V5 schema 生成 Rust/TS 绑定。每条 UI 逻辑连接固定可信 Caller 和 generation；即使 Host 刚重启且尚未 attach，用户切换也先以 Native 恢复的完整旧上下文完成持久 fence，再提交新选择。没有可发现的 AI/凭据 socket、入站监听或按用户启动的服务池。设备执行服务只绑定 authority/device，每次用户请求显式传入主体，后台核对使用任务冻结主体。

## 连接来源

| 来源 | Codex | Claude | DeepSeek |
|---|---|---|---|
| 本机已有配置 | 直接设置官方 `CODEX_HOME`，默认 `~/.codex` | 直接设置官方 `CLAUDE_CONFIG_DIR`，默认 `~/.claude` | 不提供 |
| 自定义 API | URL、API Key、模型 | URL、API Key/Auth Token、模型 | URL、API Key、模型 |

本机已有配置的登录、账号识别、凭据读取与刷新均由官方工具负责。RSS 不解析 auth.json、提取 OAuth token、访问外部工具 Keychain 或复制登录状态；模型留空时采用官方默认配置。固定 Codex 0.155.0 app-server 不支持 CLI 命名 profile，界面不提供该选项，也不实现替代配置解析器。

用户的配置目录与 RSS 运行目录分开。RSS 不向用户目录写入 config.toml；Codex 通过运行参数限制工具、hooks、plugins 和 MCP。官方配置表递归合并，因此在创建线程前读取官方配置 API，对每个非 RSS MCP 显式禁用，并校验实际工具清单。Claude 使用 user 设置来源，同时显式关闭 hooks、plugins、skills 和任意 MCP，只保留产品允许的工具。项目设置不能覆盖产品限制。恢复只使用 RSS 自己登记的原生会话 ID，不枚举或清理用户的其它会话。

自定义 API 的密钥在 AppKit 安全输入框填写，直接进入同一次验证保存操作，WebView 不持有秘密或凭据引用。Keychain 只保存一个应用级 256 位主密钥；Node 使用 AES-256-GCM（随机 96 位 IV、128 位 tag），AAD 绑定 tenant/principal/authority、connectionId 和 configRevision。密文存入现有 SQLite 连接修订行，与配置和默认偏好在同一事务提交。只使用已有配置时不访问应用 Keychain；已有密文而主密钥缺失时拒绝使用，不重建密钥或删除数据。

保存必须收到一条简短模型探针的完成结果，并确认 worker 已停止。Codex 验证采用 ephemeral 线程，Claude 使用 persistSession:false。取消、验证失败、修订冲突或用户切换不提交连接；编辑默认保留原密钥，显式选择更换才重新输入。删除在同一事务中清除该连接全部修订的密文和默认选择，保留元数据及会话历史。已经运行的 worker 可处理已接纳工作，删除后不能启动新的 worker。第一条可用连接成为默认，后续新增不替换默认，删除默认后无自动替补。

## 产品会话与交付

创建产品 Session 不启动 provider。首条输入才打开阶段，阶段固定 connectionId/configRevision；选择新连接后已接收队列按原阶段完成，下一条输入建立新阶段。每个 Session 最多一个 live worker，加应用总量上限，没有外部账号配额。原命令重试先返回旧回执；恢复失败不静默重放或换上下文。

worker 的 activation 数据通过既有私有管道传入，包含该次启动所需的配置与秘密；不写参数快照、artifact URL、公共 wire 或日志。主密钥不进入 worker。持久 launch fence 继续承担进程恢复核对，原生会话索引继续承担 RSS 自有历史定位。

历史预览只包含已完成用户输入和稳定助手文本；用户选择最近 N 轮或全部并确认。预览绑定目标配置版本、水位、命令/消息 ID 和内容哈希，不包含工具、系统指令或原始附件。设备任务始终保留冻结 actor；模型终态不等于设备业务完成。

当前无历史数据升级要求，不实现旧凭据、账号或参数快照的兼容读取、迁移和清理流程。本 PR 不新增 HMAC、防重放 nonce、凭据授权票据或 worker grant。后续 S2 独立服务/跨权限进程边界按 [#2462](https://dev.azure.com/shengming0923/rss/_workitems/edit/2462) 评估并补充必要机制；AES-GCM 随机 IV 不属于该延期范围。

## 验证与来源

`pnpm test:ai-host` 覆盖真实 SQLite、worker 进程、取消、阶段和交付恢复；`pnpm test:ai-acceptance` 使用固定 SDK/native 进程和本地模型协议服务。加密测试注入测试主密钥，不访问用户 Keychain。

`node scripts/check-native-credentials.mjs` 验证真实 WebView → AppKit 输入 → 私有通道 → Host 模型探针 → 加密 SQLite 保存和删除。验收程序注入主密钥 backend，脚本不调用系统钥匙串命令。它使用同一 clean commit 的固定 runtime artifact，输出 `.local-ci-runs/native-credentials.json`，证明本地接缝而非云端认证。

`node scripts/check-connection-sources.mjs` 把当前用户已有配置目录交给官方 Codex/Claude，发送最小真实模型请求。两个来源均须完成探针；目录缺失记 partial，认证或能力失败仍判失败。报告不包含账号、目录或秘密，该入口不纳入无凭据 CI。平台窗口验收见[桌面指南](../../docs/guides/desktop-development.md)。

来源：Rust `std::os::unix::net::UnixStream::pair`；[Codex 0.155.0 config merge](https://github.com/openai/codex/blob/rust-v0.155.0/codex-rs/config/src/merge.rs) 与 [CLI profile 入口](https://github.com/openai/codex/blob/rust-v0.155.0/codex-rs/cli/src/main.rs)；Claude Agent SDK 0.3.277 `sdk.d.ts`；[Node crypto](https://nodejs.org/api/crypto.html)；security-framework 3.5.1 `src/passwords.rs`、`src/random.rs`；objc2-app-kit 0.3.2 `NSAlert` / `NSSecureTextField`。没有复制上游认证实现。
