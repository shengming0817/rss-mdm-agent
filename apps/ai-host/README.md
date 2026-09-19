# 本地 AI Host 应用

本应用装配真实 SQLite Store、独立 provider worker 和 ACP–A2UI access service。Claude、Codex、DeepSeek Harness（DSH）均支持已有用户配置或显式 API URL/凭据文件；不限定官方域名。配置来自可信本地组合根，不能通过聊天修改。受控 S1 桌面闭环只验证 macOS arm64 的固定 Codex 0.155.0。

```sh
pnpm build:ai-host
node apps/ai-host/dist/cli.js /absolute/path/configuration.json
pnpm bundle:ai-host
.local-ci-runs/ai-host-runtime/bin/rss-ai-host /absolute/path/configuration.json
```

运行包固定 Node 24.14.1 / SQLite 3.51.2，验证 Node archive SHA-256、源码/部署 lock 及已提交源码身份，打包全部 provider 依赖；实际启用的 worker 才读取凭据与加载 SDK。运行记录与运行包在被忽略的 `.local-ci-runs` 中再生，不提交个人配置。

## 统一配置

```json
{
  "databasePath": "/private/runtime/ai.sqlite",
  "socketPath": "/private/runtime/ai.sock",
  "nativeDirectory": "/private/runtime/native",
  "workingDirectory": "/private/runtime/workspace",
  "caller": {"tenantId":"s1-test","principalId":"fixture-actor","authorityId":"desktop-fixture"},
  "session": {"provider":"codex","config":{"id":"s1-local","revision":"r1"},"accountRef":"s1-user-codex","profile":"controlled_tools"},
  "connection": {"source":"existing_user_config","directory":"/absolute/user/codex-directory"}
}
```

`provider` 为 `claude`、`codex` 或 `deepseek`。已有用户配置可显式覆盖 `connection.model` 和 `connection.profile`。只提取模型、endpoint 与认证；不导入用户 MCP、插件、shell、工具或自动批准设置。已有用户目录可为 0755，但不得被其他用户写入；秘密文件须为当前用户的普通 0600 文件。配置与自有 native/数据库/socket 目录要求私有所有权，拒绝符号链接与超限读取。

Codex 从 `config.toml` 选择 profile、model_provider 和 `auth.json`；ChatGPT 登录通过原生 `chatgptAuthTokens` 外部认证接入隔离的 CODEX_HOME，只重读同账号 access token，不复制 refresh token或修改用户登录。API key 模式支持自定义 provider URL。Claude 提取允许的 `settings.json` 连接字段；macOS 用户登录复用选定配置目录对应的 Keychain，隔离工具配置。DSH 提取 `.credentials.yaml` 的 DEEPSEEK_API_KEY、`settings.yaml` 的模型及选定 profile 的 provider/model 配置，不执行 Cordis 插件。

自定义端点将 `connection` 替换为：

```json
{
  "source":"custom_endpoint",
  "apiUrl":"https://models.example.test/v1",
  "credentialPath":"/private/runtime/api-key",
  "credentialType":"api_key",
  "model":"configured-model"
}
```

URL 允许 HTTPS 或 loopback HTTP，拒绝 URL 中的凭据/query/fragment。Claude 还支持 `auth_token` / `oauth_token`。原 `claude` 专用配置形状不保留；改变连接身份需更新配置 revision，不迁移旧身份的原生历史。

## 协议和持久交付

`conversation` 通过私有 Unix socket 提供 ACP。`controlled_tools` 另通过父进程的 stdin/stdout 建立标准 MCP；socket 与管道职责分开，不引入第三种聊天协议。必须连接真实 Rust execution-mcp 并通过固定 provider/platform verifier 后才开放工具；配置字符串不构成隔离证明。

Host-owned namespace 与 stage operationId 在工具请求前原子写入 AI 库；恢复先核实 Rust 当前事实。AI 本轮完成不终止交付；Host 重启和 provider 不可恢复时仍可处理原交付。MCP 管道断开触发 Host/worker 有界关闭，避免 Rust 宿主死亡后留下可工作的模型进程。

AI wire **4**、SQLite schema **3** 直接替换旧版，拒绝旧库，不迁移、不双读、不做旧字段 fallback。桌面接线和验证边界见[桌面指南](../../docs/guides/desktop-development.md)。
