# 桌面开发与验证

桌面生产入口装配 Rust `execution-app` / SQLite、S1 测试 runner、独立 Node AI Host 和真实 provider worker。Vue 通过固定 Tauri IPC 消费同一个执行 owner；AI 通过 Host 持久交付和 MCP stdio 调用该 owner。S1 只使用 Test authority、macOS arm64 测试设备和测试用户，不安装软件、不执行操作系统脚本、不提权，不连接企业服务。

## 启动和运行包

使用根 manifest 固定的 Node、pnpm 与 `rust-toolchain.toml`。所有命令在本仓运行。

```sh
pnpm install --frozen-lockfile
pnpm dev                 # 自动准备/复用开发 AI Host 后启动桌面
pnpm desktop:build       # 干净、已提交源码；自动打包 Node/依赖、stage、构建 .app
```

分步诊断可单独执行 `pnpm bundle:ai-host` 和 `pnpm stage:desktop-runtime`。`pnpm dev` 使用独立的 `.local-ci-runs/ai-host-dev-runtime` 和 development manifest，允许未提交源码；首次启动自动构建，后续按 Host、adapter、contract、相关 workspace 包、lock、固定 Node 和打包脚本的内容摘要判断是否重建。普通 UI 修改不触发重建。每次启动校验运行包完整性与真实 CLI 生命周期，准备失败会非零退出，不启动 Tauri。修改 Host 后重新运行 `pnpm dev`。

高级诊断可用 `RSS_AI_HOST_RUNTIME=/absolute/verified/ai-host-runtime pnpm dev` 显式选择并验证运行包。缺依赖先运行 `pnpm install --frozen-lockfile`；构建失败查看命令输出；运行包损坏时删除开发 runtime 目录后重试。准备进程异常中止留下锁时，确认没有其他准备进程后删除 `.cache/desktop-dev.lock`。开发包不进入发布 stage；release 忽略该 override，仍要求干净、已提交源码和固定候选摘要。

普通 Cargo/schema 检查使用基础 Tauri 配置，不依赖运行包；发布构建显式合并 `tauri.bundle.conf.json`。打包脚本先校验固定 Node archive、源码与部署 lock、真实 SDK 生命周期，再将通过的当前候选复制到被忽略的 resources 目录。macOS bundle 通过 `bundle.macOS.files` 整目录复制 runtime，以保留 pnpm 依赖符号链接；普通 resources 文件枚举会漏掉这些链接，不能用于该 runtime。发布应用只从自身资源目录启动 AI Host。缺失或不可用的 AI 不影响 Rust 任务读取，也不会降级为虚构对话。

第一次启动在应用数据目录 `test-users` 创建私有用户注册表、Host 路径配置、AI 库与设备 journal。用户在底部“设置”选择测试名称，再添加个人连接并验证保存；无凭据也能创建空产品会话和读取已有历史。旧 `s1/client.json` 不读取、不迁移。连接来源、版本和认证约束见 [AI Host](../../apps/ai-host/README.md)。

切换测试用户会卸载旧工作区，原生 IPC 检查捕获的 generation；迟到返回不能进入新用户视图。原有设备任务的 actor 不变，模型队列被取消；取消未确认时保留未知结果。重选已有名称保持稳定 ID，重启恢复上次选择并换新 generation。

## 执行、批准和恢复

Rust 持有唯一业务状态。`preview` 登记精确冻结计划但不提交；`submit` 单独写接纳回执；同请求同内容重放只读取原状态。编辑表单后生成新请求 ID，响应丢失时保留原 ID。任务状态包含 `submitted`，避免把预览误作提交。请求列表使用原请求 ID 游标，每次扫描最多 128 条，保留上一页/下一页；即使当前扫描页只有预览也继续提供下一页。所选任务及待核实提交/回答/批准/取消按原 ID 独立读取（每次最多 3 个），翻页和轮询不切换选择，也不因任务不在当前页而丢失详情。独立详情始终按原业务 ID 授权读取。

办公套件要求 S1 测试管理员批准。预览摘要、任务批准页和 AI 可信详情均展示 Rust 冻结的请求主体、授权域与人/AI 来源；AI 来源另展示账号/配置版本、会话和工具调用引用，与运行身份分别标明。任务详情的独立批准命令核对 actor、device、完整计划摘要及有效期，Rust 原子消费一次批准并释放一次派发。普通确认、AI 问答和 A2UI action 均不能批准。取消交互与“请求取消原任务”分开；取消回执不证明终止或回滚。维护样本保持等待；未知效果样本只核实原尝试，不自动重跑。

Rust IPC 保留 `outcomeUnknown` / `confirmationUnknown` 分类。UI 保留提交的 requestId/planId/digest 和交互 commandId，轮询原任务或重试原命令；明确拒绝才解除不确定状态。批准/取消保留原动作与精确计划，轮询同计划的权威状态后清除未确认提示，其他任务或旧计划的回执不能清除。

AI initiator 来自 Host 写入的 MCP metadata，经 Rust 与启动时绑定核对；模型参数不能覆盖 caller、provider/account/config、会话或 tool-call 来源。Human 仅与同 OS session 的 Human 任务匹配；AI 仅与同 provider、OS session、provider account/config 和 conversation 的 AI 任务匹配，不能读取、预览、提交或取消 Human 任务。UI 能读取同一业务任务和原始来源，工具返回文本仅为对话资料。

关闭窗口销毁视图并分离 ACP 连接，Rust owner 和模型工作继续。显示窗口/应用 Reopen 创建新视图并读取持久状态。明确“退出”先有界关闭 Node/worker/MCP，再停止 Rust owner，不隐式取消业务。异常退出后重新打开数据库只核实已有尝试；runner 历史丢失保留 Unknown。

Host 在发送工具操作前原子保存 delivery intent；回复丢失后按原业务 ID 和精确 plan 核实。`outcomeUnknown` 不生成完成回执；仅明确未提交才允许发送。交付可在 AI 本轮结束、provider 不可恢复或 Host 重启后继续；它不能改变模型命令或伪造 provider 结果。Rust 回执引用在 Host 落盘后才能确认。当前 MCP 查询不消费 Rust 结果日志，业务证据由 Rust 保留。

## 边界和验证

`self-service/native.ts`、`assistant/native.ts`、`settings/native.ts` 和 `test-users.ts` 只调用各自固定字面量 IPC。WebView 无网络、文件、进程、凭据或 SDK 入口；capability 只授权本地 main 窗口。原生进程、socket 和 SQLite 只在 Rust composition 层。CSP、导航拒绝、IPC ACL、源码 AST 守卫分别验证，源码守卫不是 OS 沙箱证明。fixture 自己持有测试 Human/OS session 构造，源码守卫拒绝 fixture 反向依赖 composition。

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

无凭据发布资源验收使用 `pnpm bundle:ai-host && pnpm check:desktop-bundle`，要求干净、已提交源码。入口构建实际 `.app`，在隔离 HOME 下启动生产 main，禁用 runtime override，核验完整 runtime 树与 Native 输出的 health 握手结果；结果写入 `.local-ci-runs/desktop-bundle.json`。此 smoke 只证明启动与资源定位，使用隔离进程组清理，不作为优雅退出证据。

真实 macOS arm64 桌面验收单独运行 `pnpm bundle:ai-host && pnpm check:desktop-native`，消费同一提交的固定 runtime 和现有 Codex 配置。实际 WebView 验收覆盖首次配置、关闭/重开、用户隔离、授权历史、Host-only 重启、设备任务事实保持，以及重启后的真实新对话。结果写入 `.local-ci-runs/desktop-native.json`，绑定源码、lock、配置模式和 runtime manifest；不属于无凭据 CI，不证明真实 OS 效果。

真实验收不指定模型，由官方工具从本机已有配置解析默认模型；复用已有用户登录，不修改用户配置或静默换模型。默认模型仍须通过受控工具探针；需要 code-mode host 的模型不会自动降级为其他模型。回执记录 `official_configuration_default`，不猜测模型名称。

## 设置、诊断和 AI 恢复

设置入口始终可达，未选择用户或 AI Host 不可用时仍可查看诊断与关于。首次路径是用户名、连接验证保存、新建对话；“稍后配置”保留空产品会话和自助入口。常规与通知仅说明已有行为，不提供无底层服务的开关。

Native 长期持有用户、一个设备执行服务和应用主密钥访问 owner；Host 进程可以单独替换。启动前核验关键文件、平台和契约版本，配置、存储和恢复完成后的私有 health 应答才表示 ready。运行包不匹配直接拒绝；release 构建忽略开发 override。关闭/回收旧 Host 后才启动新代，worker fence 未解决时禁止重复 worker。

Native 在每次启动前按打包端同一规则重算文件字节、权限和符号链接图摘要；`.app` 候选的预期摘要由 stage 后的 Native 构建绑定，改写资源目录中的 manifest 不能替换它。显式开发 override 校验其 manifest 与实际树一致，信任由开发者选择该路径建立。health 超时或不合法时先撤销 control/MCP，再有界停止并回收进程；回收未知保留原 owner 阻断新代。

“重新连接”只建立视图通道并核对历史，不重启 Host，不重发请求。“重启 AI Host”有明确提示，模型请求可能中断，设备任务继续；它不调用设备服务关闭，也不记录虚假的任务完成或取消。Host 不可用时仅显示当前用户已加载的只读历史；未加载历史在恢复后按原接口分页读取，切用户清空全部旧视图。不存在 Native SQLite 历史旁路。

连接操作区分配置、认证、能力、限额、拒绝、取消和未确认结果。仅明确认证错误提示更新凭据；普通失败不伪装为认证失效。普通对话运行真实文本探针，受控用途还要求所选模型调用验证专用无副作用工具；探针不接设备执行服务。只有验证及进程退出确认后才事务保存。Codex 禁止模型 fallback，并核对返回模型；已有配置的默认模型由官方工具解析。

诊断只包含闭集阶段/错误码、时间、运行版本和资源来源类别，最多保留 64 条故障记录。通过原生保存对话框导出，不包含凭据、原始错误、端点、个人路径、对话或数据库内容。缺包按开发/发布来源分别提示构建或重装，旧进程回收未确认则保留阻断。

Native/Host 私有协议直接切换为 2，旧候选不能通过就绪校验。Host 启动/清理诊断使用 runtime schema 生成的 `hostProcessDiagnostic` 有界帧，旧 stderr 文本不再识别；Native 控制出站及状态均直接构造生成类型。HTTP 错误证据按 dispatch 归属且只消费一次，重叠 dispatch 或多个 HTTP 请求的归属不明确时保留 adapter 自身的闭集结果，不猜测认证原因。

设计参考：Microsoft [设置指南](https://learn.microsoft.com/en-us/windows/apps/design/app-settings/guidelines-for-app-settings) 与 [WinUI Gallery SettingsPage.xaml](https://github.com/microsoft/WinUI-Gallery/blob/main/WinUIGallery/Pages/SettingsPage.xaml)。Vue/Tauri 保持现有技术栈。
