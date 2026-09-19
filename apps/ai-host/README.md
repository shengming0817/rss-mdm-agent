# 本地 AI Host 应用

本应用把真实 SQLite Store、独立 worker、Claude adapter 和既有 ACP–A2UI access service 装配为私有本地 socket 服务。配置文件是可信组合输入，不由聊天协议接纳。socket 断开只 detach；SIGINT / SIGTERM 请求 Host 有界关闭。

```sh
pnpm build:ai-host
node apps/ai-host/dist/cli.js /absolute/path/configuration.json
pnpm bundle:ai-host
.local-ci-runs/ai-host-runtime/bin/rss-ai-host /absolute/path/configuration.json
```

最后一条入口使用运行包内固定 Node 24.14.1（SQLite 3.51.2），不依赖系统 Node。构建校验官方 Node archive SHA-256，安装本次 tarball 及锁定依赖，并记录源码 SHA、lock hash、平台与产物 hash。运行包及证据可再生，位于 Git 忽略的 `.local-ci-runs`。目前仅 macOS arm64 通过运行包门禁。

配置示例（路径须由部署方替换为实际绝对路径）：

```json
{
  "databasePath": "/private/runtime/host.sqlite",
  "socketPath": "/private/runtime/host.sock",
  "caller": {
    "tenantId": "local",
    "principalId": "operator",
    "authorityId": "local-login"
  },
  "session": {
    "provider": "claude",
    "config": { "id": "claude", "revision": "1" },
    "accountRef": "account",
    "profile": "conversation"
  },
  "workingDirectory": "/private/project",
  "claude": {
    "configurationDirectory": "/private/claude",
    "apiUrl": "https://api.anthropic.com",
    "credentialPath": "/private/credential",
    "credentialType": "api_key"
  }
}
```

配置和凭据文件需属于运行用户、为普通文件且禁止 group/other 访问；数据库与 socket 目录为私有目录。Caller 由本地账户和私有 socket 的访问边界固定，不能从消息、模型、action 或工具参数覆盖。凭据在 worker 激活后由其读取，parent 不加载 SDK 或凭据。`credentialType` 也支持 `auth_token`；可选 `claude.model` 固定模型选择。

CLI 提供 conversation profile。需要 controlled_tools 的组合根使用库接口注入真实 ToolEndpoint 与可信平台 verifier；不会用配置字符串冒充隔离证明。运行目录、账号数据和凭据不进入发布包或仓库。外部模型连通性由 adapter 的独立 smoke 流程记录。

协议消费使用 `@rss-mdm-agent/ai-client` 的 `RuntimeClient` 和 `ndJsonStream`；服务端复用 A04。此入口不新增聊天协议，也不包含 Tauri 窗口与安装器装配。
