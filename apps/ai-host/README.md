# 本地 AI Host 应用

本应用装配 V5 契约、SQLite schema 4、独立 provider worker 和 ACP–A2UI 服务。原生桌面持有测试用户选择、IPC generation、Keychain 与设备执行句柄；Host 持有个人连接、产品 Session、provider 阶段及持久交付。当前验证平台为 macOS arm64。

```sh
pnpm build:ai-host
pnpm bundle:ai-host
pnpm desktop:build
```

桌面在新的 `test-users` 数据根生成仅含路径的 `host.json`：`version: 1`、`databasePath`、`socketPath`、`credentialSocket`、`usersPath`、`nativeDirectory` 和 `workingDirectory`。原生管道提供 Rust MCP，私有 socket 的 native ingress 先核对注册表中的 generation，再注入当前 caller。WebView 不能选择 principal。`s1/client.json`、旧 caller/session 配置、明文凭据路径及旧数据库均不读取、不迁移。

## 连接来源

个人连接通过桌面面板创建、验证和保存；不编辑 bootstrap JSON。配置包含命名 connectionId、provider、configRevision、credentialRevision、opaque accountRef、credentialRef、profile 和明确的 source。秘密不进入 wire、SQLite、配置快照或日志。

| 来源 | Codex | Claude | DeepSeek |
|---|---|---|---|
| 自定义 API | URL、API Key、模型 | URL、API Key/Auth Token/OAuth Token、模型 | URL、API Key、模型 |
| 已有 CLI 登录 | `config.toml` 存储策略及 native broker 获取的 ChatGPT token | 固定 SDK 原生 Keychain source，核对 accountInfo 与 init source | 不提供 |
| 已有 CLI API 配置 | profile/model provider 与显式 API 认证字段 | settings 中显式 API key/auth token | 不提供 |

自定义凭据通过 AppKit 安全字段进入 Keychain，网页只得到按测试用户隔离的随机引用。URL 允许 HTTPS 或 loopback HTTP，拒绝 URL 凭据、query 和 fragment。用户配置目录不得由其他用户写入；含秘密的文件必须为当前用户的私有普通文件，拒绝符号链接和超限读取。不导入用户工具权限、插件、任意 MCP 或自动批准。

保存前启动固定 provider，并发送一条简短测试请求，收到完成结果后原子激活新版本；请求可能产生服务费用。失败保留旧连接。第一条可用连接成为默认，后续新增不替换默认；删除默认会清空选择，无自动替补。删除保留不可变修订和所有会话历史。

Codex 0.155.0 遵循 file/keyring/auto/ephemeral 存储策略；auto 仅在 Keychain 条目不存在时尝试文件，权限错误不回退。登录通过 `chatgptAuthTokens` 接入隔离目录，不复制 refresh token、不修改原登录。刷新只重读同一来源、同一账号且发生变化的 access token。

Claude SDK 0.3.277 / CLI 2.1.277 使用独立阶段配置目录，通过 `CLAUDE_SECURESTORAGE_CONFIG_DIR` 选择实际 Keychain source。默认源使用空值，显式目录按上游 NFC/hash 规则寻址；OS 用户名用于 Keychain account。`accountInfo()` 与 init 的认证来源共同核对，观察到的账号信息与配置修订绑定；email 是观察字段，不宣称为稳定 provider 主键。已有登录不使用环境 OAuth token 替代。所有阶段继续封闭设置、插件和工具旁路。

## 产品会话与交付

创建产品 Session 不启动 provider。首条输入才打开阶段；阶段固定连接和凭据修订。选择新连接后已接收队列先完成，期间拒绝新的普通输入；下一条输入建立新阶段。显式新上下文意图不建立额外任务。原命令重试先返回旧回执，不重新创建阶段。原生恢复失败需要用户明确选择恢复或新上下文，不静默重放。

历史预览只包含已完成用户输入和稳定助手文本；用户选择最近 N 轮或全部并确认。预览绑定目标修订、水位、命令/消息 ID 和内容哈希，不包含工具、系统指令或原始附件，不截断。回执保存接纳阶段，跨阶段设备交付继续用原 caller/binding。

同一设备 journal 由不可变用户执行句柄共享。切换用户终止旧 UI 连接，取消旧模型工作，未确认取消保持未知；设备工作保持原 actor。后台交付先保存 intent，丢失回复后核实原业务 ID 和精确计划。AI terminal 不等于业务完成。

## 验证

`pnpm test:ai-host` 覆盖真实 SQLite、worker 进程、取消、阶段和交付恢复；`pnpm test:ai-acceptance` 使用固定 SDK/native 进程和本地模型协议服务，不代表真实云端凭据验收。

显式执行 `node scripts/check-connection-sources.mjs` 使用生产 Rust broker、当前 OS 用户已有 Codex/Claude 来源和真实模型探针；它只输出来源与闭合结果码，不输出路径、账号或秘密。缺失或不可用来源不能算通过。此入口不纳入无凭据 CI。原生窗口与平台凭据入口另按[桌面指南](../../docs/guides/desktop-development.md)验收。

上游依据：Codex 0.155.0 `codex-rs/login/src/auth/storage.rs`（commit `f0a1b8f0849d90960bc406b848f32e5a129b0457`）；Claude Agent SDK 0.3.277 `sdk.mjs` / bundled CLI（secure storage selector、accountInfo）；security-framework 3.5.1 `src/passwords.rs`；objc2-app-kit 0.3.2 `NSAlert` / `NSSecureTextField`。
