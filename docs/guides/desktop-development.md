# 桌面开发与验证

桌面生产入口装配 Rust `execution-app` / SQLite、S1 测试 runner、独立 Node AI Host 和真实 provider worker。Vue 通过固定 Tauri IPC 消费同一个执行 owner；AI 通过 Host 持久交付和 MCP stdio 调用该 owner。S1 只使用 Test authority、macOS arm64 测试设备和测试用户，不安装软件、不执行操作系统脚本、不提权，不连接企业服务。

## 启动和运行包

使用根 manifest 固定的 Node、pnpm 与 `rust-toolchain.toml`。所有命令在本仓运行。

```sh
pnpm install --frozen-lockfile
pnpm bundle:ai-host       # 要求干净、已提交源码
pnpm stage:desktop-runtime
# 开发时显式指定已验证的 artifact 绝对目录；不自动扫描开发产物
RSS_AI_HOST_RUNTIME=/absolute/verified/ai-host-runtime pnpm dev
pnpm desktop:build       # macOS arm64 .app，内含固定 Node 与依赖闭包
```

普通 Cargo/schema 检查使用基础 Tauri 配置，不依赖运行包；发布构建显式合并 `tauri.bundle.conf.json`。打包脚本先校验固定 Node archive、源码与部署 lock、真实 SDK 生命周期，再将通过的当前候选复制到被忽略的 resources 目录。发布应用只从自身资源目录启动 AI Host。缺失或不可用的 AI 不影响 Rust 任务读取，也不会降级为虚构对话。

第一次启动在应用数据目录 `s1/client.json` 创建私有配置。默认 Codex `controlled_tools` 使用已有用户配置（启动环境 `CODEX_HOME` 或用户默认目录），不修改原始登录和权限设置。支持的连接方式和三引擎配置见 [AI Host](../../apps/ai-host/README.md)。改变账号、模型或 endpoint 时必须更新配置 revision；原生历史在原账号/配置身份下读取，不能换身份继续旧线程。

## 执行、批准和恢复

Rust 持有唯一业务状态。`preview` 登记精确冻结计划但不提交；`submit` 单独写接纳回执；同请求同内容重放只读取原状态。编辑表单后生成新请求 ID，响应丢失时保留原 ID。任务状态包含 `submitted`，避免把预览误作提交。请求列表当前每页上限 128；独立详情始终按原业务 ID 授权读取。

办公套件要求 S1 测试管理员批准。任务详情的独立批准命令核对 actor、device、完整计划摘要及有效期，Rust 原子消费一次批准并释放一次派发。普通确认、AI 问答和 A2UI action 均不能批准。取消交互与“请求取消原任务”分开；取消回执不证明终止或回滚。维护样本保持等待；未知效果样本只核实原尝试，不自动重跑。

AI initiator 来自 Host 写入的 MCP metadata，经 Rust 与启动时绑定核对；模型参数不能覆盖 caller、provider/account/config、会话或 tool-call 来源。UI 能读取同一业务任务和原始来源，工具返回文本仅为对话资料。

关闭窗口销毁视图并分离 ACP 连接，Rust owner 和模型工作继续。显示窗口/应用 Reopen 创建新视图并读取持久状态。明确“退出”先有界关闭 Node/worker/MCP，再停止 Rust owner，不隐式取消业务。异常退出后重新打开数据库只核实已有尝试；runner 历史丢失保留 Unknown。

Host 在发送工具操作前原子保存 delivery intent；回复丢失后按原业务 ID 和精确 plan 核实。`outcomeUnknown` 不生成完成回执；仅明确未提交才允许发送。交付可在 AI 本轮结束、provider 不可恢复或 Host 重启后继续；它不能改变模型命令或伪造 provider 结果。Rust 回执引用在 Host 落盘后才能确认。当前 MCP 查询不消费 Rust 结果日志，业务证据由 Rust 保留。

## 边界和验证

只有 `self-service/native.ts` 和 `assistant/native.ts` 可调用各自固定字面量 IPC。WebView 无网络、文件、进程、凭据或 SDK 入口；capability 只授权本地 main 窗口。原生进程、socket 和 SQLite 只在 Rust composition 层。CSP、导航拒绝、IPC ACL、源码 AST 守卫分别验证，源码守卫不是 OS 沙箱证明。

Rust 命令、task details 和 MCP schema 生成前端/模型声明，无手写第二份 wire：

```sh
node scripts/check-self-service-bindings.mjs --write
node scripts/check-execution-bindings.mjs --write
node scripts/check-self-service.mjs --write  # 仅浏览器只读 fixture
cargo test -p rss-mdm-desktop --test composition --locked
pnpm test:ai-host
pnpm check:boundaries
make ci CI_BASE=origin/develop
```

浏览器未在 Tauri 环境运行时只显示明确的静态样本，写入口禁用。静态样本不作为真实桌面/AI 验收。真实模型与原生窗口验收仅覆盖 macOS arm64、固定 Codex 0.155.0；不要求 Windows/Linux 或三个引擎完成同一 E2E。S2 真实平台执行、安装签名、公证、升级及 T3 企业身份仍在本次范围外。

来源：Tauri `crates/tauri/src/app.rs` / `webview/webview_window.rs` @ 2.11.2；runtime-wry `src/lib.rs` @ 2.11.4（最后窗口销毁与 ExitRequested）；rmcp `src/model/meta.rs` @ 3.4.0（request metadata）；MCP TypeScript SDK `client/index.ts` / `shared/stdio.ts` @ 1.30.0。

真实 macOS arm64 桌面验收使用 `pnpm bundle:ai-host && pnpm check:desktop-native`，要求源码已提交且工作树干净。入口构建实际 WebView 并消费固定 runtime，使用现有 Codex 用户登录；结果写入 `.local-ci-runs/desktop-native.json`，绑定源码、lock 和 runtime manifest。每次使用全新私有目录，窗口销毁后重新连接同一后端，再从可信任务详情批准测试计划。该验收不属于无凭证 CI，也不证明真实 OS 效果。
