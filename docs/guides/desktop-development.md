# 桌面开发与验证

桌面生产入口通过认证 IPC 消费 `apps/agent-service`，不持有执行 SQLite 或 S1 runner。Vue 和 AI MCP 共享后台任务引用；系统服务持有唯一执行 intent、恢复和结果送达状态。独立 Node AI Host 与 provider worker 只承接会话，不签出企业批准。后台企业任务授权和来源由服务端决定；客户端校验签名、材料、设备注册及 OS 会话。

## 启动和运行包

使用根 manifest 固定的 Node、pnpm 与 `rust-toolchain.toml`。所有命令在本仓运行。

```sh
pnpm install --frozen-lockfile
pnpm dev                 # 自动准备/复用开发 AI Host 后启动桌面
pnpm desktop:build       # 自动打包 Node/依赖、stage、构建 .app
```

分步诊断可单独执行 `pnpm bundle:ai-host` 和 `pnpm stage:desktop-runtime`。`pnpm dev` 使用独立的 `.local-ci-runs/ai-host-dev-runtime` 和 development manifest，允许未提交源码；首次启动自动构建，后续按 Host、adapter、contract、相关 workspace 包、Rust execution schema 与绑定检查、lock、固定 Node 和打包脚本的内容摘要判断是否重建。普通 UI 修改不触发重建。每次启动校验运行包完整性、manifest 与本轮源码摘要一致性及真实 CLI 生命周期，准备失败会非零退出，不启动 Tauri。修改 Host 后重新运行 `pnpm dev`。终端断开（SIGHUP）、中断（SIGINT）与停止（SIGTERM）均转发到独立开发进程组并有界清理。

高级诊断可用 `RSS_AI_HOST_RUNTIME=/absolute/verified/ai-host-runtime pnpm dev` 显式选择并验证运行包。缺依赖先运行 `pnpm install --frozen-lockfile`；构建失败查看命令输出；运行包损坏时删除开发 runtime 目录后重试。准备进程异常中止留下锁时，确认没有其他准备进程后删除 `.cache/desktop-dev.lock`。开发包不进入发布 stage；release 忽略该 override，并校验实际运行包完整性。

普通 Cargo/schema 检查使用基础 Tauri 配置，不依赖运行包；发布构建显式合并 `tauri.bundle.conf.json`。打包脚本先校验固定 Node archive、锁定部署依赖、真实 SDK 生命周期，再将通过的当前候选复制到被忽略的 resources 目录。macOS bundle 通过 `bundle.macOS.files` 整目录复制 runtime，以保留 pnpm 依赖符号链接；普通 resources 文件枚举会漏掉这些链接，不能用于该 runtime。发布应用只从自身资源目录启动 AI Host。缺失或不可用的 AI 不影响 Rust 任务读取，也不会降级为虚构对话。

第一次启动在应用数据目录 `desktop` 创建私有用户注册表、Host 路径配置和 AI 库；执行 journal 只属于系统服务。旧 `test-users` 数据原样保留，不自动迁移。用户在底部“设置”选择测试名称，再保存个人连接后单独测试；无凭据也能进入空白对话、保留草稿和读取已有历史。旧 `s1/client.json` 不读取、不迁移。AI 会话协议和数据库格式升级后明确拒绝旧格式，保留旧文件，不自动迁移或重建；只有显式 `native-e2e` 测试构建支持 `--test-data-dir <新的绝对目录>`，旧历史不能直接导入。连接来源、版本和认证约束见 [AI Host](../../apps/ai-host/README.md)。

切换测试用户会卸载旧工作区，原生 IPC 检查捕获的 generation；迟到返回不能进入新用户视图。原有设备任务的 actor 不变，模型队列被取消；取消未确认时保留未知结果。重选已有名称保持稳定 ID，重启恢复上次选择并换新 generation。

## 企业、测试与不登录入口

“设置 → 账户入口”始终保留企业登录、测试用户和“不登录使用”。不登录使用创建稳定的本地访客，支持个人 AI；设备操作仍须来自后台任务；访客、测试用户和企业账号的数据互相隔离。测试用户仍按本地名称选择，不代表认证。

企业登录先添加组织名称、HTTPS 服务 origin 与租户 UUID，再选择组织并填写本地账号。原生安全输入框会展示实际服务、租户和账号；密码只由原生进程发送到该服务，不进入 WebView、不保存。仅接受系统信任的 HTTPS，不跟随重定向，不接受 URL 中的凭据、路径或查询。组织配置不构成授权；切换连接必须重新登录，随后分别查询 Identity 会话与 rss-mdm 当前业务授权，核对实例/租户/主体，并要求至少一个有效业务 grant。企业执行走已注册系统服务，企业批准不在客户端重新签发；本地 AI 的产品归属不随 provider 账号/API key 变化。

本轮使用产品已提供的 Identity v2 本地密码认证与 rss-mdm 业务授权 v1 HTTP 接口。系统浏览器 OIDC 到原生客户端的凭据交接尚无公开契约，因此此入口不支持 OIDC-only 账号。组织连接列表保存在本机；它不是服务端组织成员目录，也不自动发现或认领组织。

企业 cookie/CSRF 只保存在原生进程内，重启必须重新登录，不支持离线授权缓存。AI 连接、发送、接收及连接保存前在线复核；原生进程每 2 秒发起被动复核，每个 HTTP 请求最多 8 秒（依次核对认证和授权），不延长 idle 期限。撤销在下一次成功核查时发现，断网/核查失败即撤销本地上下文；不宣称服务端推送式瞬时撤销。已知过期时间也在原生 IPC 与 Host caller 准入处检查。退出或切换先关闭旧视图、取消旧模型工作；清理失败停止 Host，新账号不能复用旧通道。服务端注销无法确认时本地仍退出，并提示通过企业账户管理撤销服务端会话。已登记测试设备任务保留原 actor 和运行事实，不因账户切换伪造取消。

企业 AI 历史按服务地址与实例绑定的 authority、租户、主体三个标识归属；显示名、用户名、email 与 provider 凭据均不能认领旧数据。现有测试数据原地保留，只能回到原测试用户读取。本轮不迁移、不丢弃，也不提供跨账户导入/导出；未来迁移必须通过单独的显式选择与授权流程，不能直接修改用户名或数据库 owner。访客退出后再次选择“不登录使用”仍回到同一本机访客数据。

## 执行动作、确认和恢复

系统服务的 Rust journal 持有唯一执行状态。桌面展示后台 Offer；用户确认后按 task、attempt、revision 和原 request 请求执行。MCP `execution_tasks` 读取同一批任务，`execution_execute` 仅提议精确任务，主动安装仍由用户在桌面确认。没有本地目录转计划或任意脚本入口。提交超时后查询原 request，不能换 ID 重派。

人工动作在已有权限内由本人确认。AI 动作由可信策略按真实内容和运行上下文分为 0/1/2/3：0/1 在允许规则内直接执行，2 由当前有执行权限的用户确认，3 与未知默认阻止。明确禁止或无权限不能通过确认放行；AI 经用户确认后仍保留 AI 来源。Policy 保持独立非交互授权。产品确认、provider 权限与 OS 同意分别执行，模型回答不能代替产品确认。

桌面显示动作、来源、目标、运行身份、风险等级与内容摘要。确认有效期最长一分钟且不超过原执行有效期，等待不消耗进程运行预算。确认、取消及响应丢失均保留原请求身份，不重新创建任务；重启和 Unknown 只核实原 attempt。请求列表按原请求 ID 分页，每页最多 128 条，所选任务独立读取。

正式装配使用本地执行 V5、IPC V5、SQLite schema 6；旧库明确拒绝并保持原文件，不迁移、清空或新建 journal 绕过未决任务。生产启动失败直接显示诊断，不引导切入测试目录。测试专用构建和 S1 样本不代表生产结果。

AI 来源由 Host metadata 与 Rust 当前用户绑定核验，不能通过工具参数改为 Human。UI 确认原请求时单独核验当前用户权限，执行来源保持不变。

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
make ci CI_BASE=origin/develop # 按影响范围；ci-full 强制全量，ci-plan 查看计划
```

macOS 的正式 `make ci` / `make ci-full` 对整个 Node 与 Rust 检查过程取得 worktree 独占租约，并将 Cargo `target` 放进默认四槽的本机缓存池；同一 worktree 优先复用槽位，槽位换属时清理旧产物。`AGENT_TARGET_POOL_N` 可设置 1–32 个槽，`AGENT_TARGET_POOL_ROOT` 可指定专用空目录或已标记的池目录。分配器短时竞争最多等待 2 秒；槽满或同一 worktree 已有受管运行时立即失败，原运行结束后重试。`make ci-plan` 不占槽；Windows 仍使用原入口。受管运行拒绝外部 `CARGO_TARGET_DIR`、`CARGO_BUILD_TARGET_DIR` 和自定义 rustc wrapper。

定向 Rust 验证可用 `python3 scripts/build-run.py -- cargo test --locked -p <包名>` 复用同一槽位机制。直接运行 `cargo` 使用仓内 `target`；直接运行 `pnpm` 或 `cargo` 不取得 CI 的 worktree 租约，避免与同一 checkout 的正式 CI 同时修改仓内 Node 产物或运行回执。旧仓内 `target` 不自动迁移或删除。

浏览器未在 Tauri 环境运行时只显示明确的静态样本，写入口禁用。静态样本不作为真实桌面/AI 验收。真实模型与原生窗口验收仅覆盖 macOS arm64、manifest 固定的 Codex；不要求 Windows/Linux 或三个引擎完成同一 E2E。S2 真实平台执行、安装签名、公证、升级及 T3 企业身份仍在本次范围外。

来源：Tauri `crates/tauri/src/app.rs` / `webview/webview_window.rs` @ 2.11.2；runtime-wry `src/lib.rs` @ 2.11.4（最后窗口销毁与 ExitRequested）；rmcp `src/model/meta.rs` @ 3.4.0（request metadata）；MCP TypeScript SDK `client/index.ts` / `shared/stdio.ts` @ 1.30.0。

自动回归由本地 CI 的单元、协议、SQLite、adapter 和浏览器检查承担，不要求系统授权弹窗。`pnpm check:desktop-native --visual` 是额外的原生视觉验收，从仓库根启动 `pnpm dev`，加载产品 main 和真实 WKWebView。它复用既有 assistant fixture 装配同一个产品 App：资源快照和 AI Host 为显式测试来源，任务阶段、过程与效果取自 Rust 生成的测试投影，AI 关联为测试绑定。它不连接云端模型、不执行设备操作，不证明生产执行授权或效果。

该路径限定 macOS arm64，需要已解锁桌面及 System Events 辅助功能权限；环境不满足直接失败。检查浅深主题、1100×760 / 1440×960 / 480×400、会话与资源布局、任务抽屉/并排、原生键盘与焦点、关闭重开。存在已启用的中文拼音输入源时，用物理键验证候选确认不发送及显式发送，并恢复原输入源；系统主题同样在结束后恢复。无实际入口或环境的缩放、其它显示比例及 Windows 项目明确标记未验证。

脚本仍保留默认执行旅程，但它在生产执行服务接入后依赖已移除的桌面 S1 journal，尚不能作为通过的原生执行验收；迁移到独立受保护服务由 #2593 跟踪。原生视觉通过不替代该项。调试构建使用独立 macOS 文件钥匙串，退出后删除并核对默认钥匙串与搜索列表未改变；不访问个人登录钥匙串的应用主密钥。正式签名应用的钥匙串授权另行验收。

驱动使用固定 `webdriverio@9.32.0` 与 `tauri-plugin-wdio-webdriver@1.4.0` 的标准 WebDriver 接口。`native-e2e` 只允许调试构建，并要求显式 nonce、动态 loopback 端口和隔离 `--test-data-dir`；release 携带该 feature 会编译失败。脚本核对主进程与监听端口归属；固定的 [本地上游补丁](../../vendor/tauri-plugin-wdio-webdriver/NOTICE.md) 对每个请求校验运行凭据，缺失或错误凭据一律拒绝，启动日志仅记录凭据摘要。脚本不调用 WebView 内部 IPC、不替换业务回复。视觉模式的关闭/重开通过 WebDriver 原生 `window.close()` 与应用“显示窗口”菜单验证；开发窗口的标题栏关闭按钮未向 AX 暴露，物理按钮点击单独标为未验证。Tab、Shift+Tab、Escape 和应用菜单操作由 macOS System Events 发出；WebDriver 键盘事件不能替代这些证据。不要在验收期间修改源码或并发启动同一 worktree 的开发服务。

`.local-ci-runs/desktop-native.json` 记录实际模式、源码/锁文件/运行包摘要、进程归属与完成项，截图和脱敏日志在同目录。失败回执不能解释为通过。该路径证明真实 CLI、WebView 与 S1 接缝，不证明真实模型能力或设备 OS 效果。

发布资源验收使用 `pnpm bundle:ai-host && pnpm check:desktop-bundle`。入口构建实际 release `.app`，使用隔离数据目录启动生产 main，禁用 runtime override，不包含原生驱动，核验完整 runtime 树与 Native health 握手；结果写入 `.local-ci-runs/desktop-bundle.json`，绑定源码、锁文件和 runtime manifest。此 smoke 只证明启动与资源定位，使用隔离进程组清理，优雅退出由日常原生验收验证。

真实模型仍单独运行 `pnpm smoke:codex`，按 [AI Host](../../apps/ai-host/README.md) 的显式配置入口选择认证、端点与模型；它保留跨轮上下文证据，不由本地协议夹具替代。真实模型、原生 WebView 和 release 资源启动是三份不同范围的回执。

来源：WebdriverIO `desktop-mobile/packages/tauri-plugin-webdriver` @ `wdio-tauri-service@v1.4.0` (`aef40049a9c566e72de4ffd08e08197ff32386ed`)；[Tauri WebDriver 指南](https://v2.tauri.app/develop/tests/webdriver/)。

## 设置、诊断和 AI 恢复

设置入口始终可达，未选择用户或 AI Host 不可用时仍可查看诊断与关于。账户区域区分网络不可用、限流、账号/授权失效及服务响应不兼容；成功进入新的账户后清除旧原因。首次路径是选择账户入口、保存配置、测试连接、发送首条消息；“开始对话”与“稍后配置”都只进入本地空白页，首条消息才创建产品会话。常规与通知仅说明已有行为，不提供无底层服务的开关。

Native 长期持有用户、一个设备执行服务和应用主密钥访问 owner；Host 进程可以单独替换。启动前核验关键文件、平台和契约版本，配置、存储和恢复完成后的私有 health 应答才表示 ready。运行包不匹配直接拒绝；release 构建忽略开发 override。关闭/回收旧 Host 后才启动新代，worker fence 未解决时禁止重复 worker。

Native 在每次启动前按打包端同一规则重算文件字节、权限和符号链接图摘要；`.app` 候选的预期摘要由 stage 后的 Native 构建绑定，改写资源目录中的 manifest 不能替换它。显式开发 override 校验其 manifest 与实际树一致，信任由开发者选择该路径建立。health 超时或不合法时先撤销 control/MCP，再有界停止并回收进程；回收未知保留原 owner 阻断新代。

“重新连接”只建立视图通道并核对历史，不重启 Host，不重发请求。“重启 AI Host”有明确提示，模型请求可能中断，设备任务继续；它不调用设备服务关闭，也不记录虚假的任务完成或取消。Host 不可用时仅显示当前用户已加载的只读历史；未加载历史在恢复后按原接口分页读取，切用户清空全部旧视图。不存在 Native SQLite 历史旁路。

连接操作区分配置、认证、能力、限额、拒绝、取消和未确认结果。仅明确认证错误提示更新凭据；普通失败不伪装为认证失效。普通对话运行真实文本探针，受控用途还要求所选模型调用验证专用无副作用工具；探针不接设备执行服务。配置先独立保存为未验证修订；测试及进程退出确认后才追加可用修订，失败保留已保存配置和凭据。Codex 禁止模型 fallback，并核对返回模型；已有配置的默认模型由官方工具解析。

诊断只包含闭集阶段/错误码、时间、运行版本和资源来源类别，最多保留 64 条故障记录。通过原生保存对话框导出，不包含凭据、原始错误、端点、个人路径、对话或数据库内容。缺包按开发/发布来源分别提示构建或重装，旧进程回收未确认则保留阻断。

私有通信遵循唯一生成契约，无法归属到具体请求的错误不能被猜测为认证失败。

设计参考：Microsoft [设置指南](https://learn.microsoft.com/en-us/windows/apps/design/app-settings/guidelines-for-app-settings) 与 [WinUI Gallery SettingsPage.xaml](https://github.com/microsoft/WinUI-Gallery/blob/main/WinUIGallery/Pages/SettingsPage.xaml)。Vue/Tauri 保持现有技术栈。

## 助手开发

```sh
pnpm dev:assistant-fixture
pnpm check:assistant
```

打开 fixture 打印的 loopback 地址；它加载实际产品 App，使用同进程 FakeHost、实际 Rust/SQLite S1 样本，不连接真实模型或执行 OS 操作。macOS 默认使用系统 Chrome，其它环境通过 `AI_BROWSER_PATH` 指定 Chromium。测试失败不会降级为静态渲染。

手工体验设备执行时，进入 AI 页，在“更多 → 会话详情与诊断 → 按执行编号查询”的“执行请求编号”中输入 `request-1`，读取 S1 授权详情；该请求仅存在于 fixture。

品牌、工作区信息和账户入口位于统一主导航与最近会话侧栏；有效主体进入后默认打开 AI。窄窗口通过“打开主导航”访问同一组入口，设置与账户操作位于侧栏底部。任务详情在默认窗口使用右抽屉，宽窗口按需并排；未接线入口明确禁用。导航切换保留会话控制器、草稿与各会话阅读位置；用户工作区销毁才清空缓存和回调。同用户重连保留已加载历史和未知命令身份，清理旧权限回调与订阅，在权威恢复完成前只读，不自动提交。当前会话先恢复，其余按选择恢复。

个人连接从“设置 → AI 连接”先保存配置，再单独测试连接。保存不发送模型请求；测试可能产生费用，失败仍可编辑保存。AI 空白页使用中心输入卡、提问引导和真实最近会话继续入口；引导只填入空草稿，不自动发送。开始对话后使用消息时间线和底部输入框；最近对话显示首条问题的摘要，按活动时间排序，窄窗口改用抽屉。顶部只选择已就绪连接，新上下文和历史带入预览放在连接菜单内。首次创建和发送分别使用稳定身份；未知结果只核对原请求，创建期间编辑草稿或切换会话不会后台发送。运行中可编辑、排队发送；仅提供方支持时显示“调整当前任务”。API 密钥在表单密码框填写，支持粘贴与显示/隐藏，提交或离开设置页后清空；provider、端点或凭据种类变化必须重新输入。切换会话后丢弃旧历史预览结果，切换连接等待旧队列完成。

助手回答支持列表、代码及表格排版，代码块可复制；链接仅显示文字与可复制地址，图片不自动加载。复制不可用时按提示选中文本手动复制。长内容默认展开，可主动收起。

设备任务卡与其详情共用同一次授权读取；“刷新状态”核对原请求，失败保留上次记录并提示，不表示实时进度。停止回复只作用于模型，取消设备任务进入原有任务入口。

AI 文本不能覆盖设备任务事实；执行详情来自 Rust 的授权读取。普通回答与 A2UI 动作均不能签发批准。未知接纳重试保留原命令和期限，回执不等于模型终态。过期、旧 generation 和删除卡片禁止继续提交；渲染失败保留只读内容。

新的 `execution_execute` 与 `execution_cancel` 在 Host 投递前请求“允许一次 / 拒绝一次”；三个读工具不询问。相同待决提案共用一次询问，已有持久投递恢复不重问。许可过期、会话/命令结束或用户切换后不能写新执行意图。该许可仅允许 AI 发起请求，Rust 仍独立验证设备授权和具体动作确认。时间线的设备卡只从 Host 投递事件定位，再核对 Rust 返回的请求、会话和工具身份；模型文字不能生成可信设备卡。

交互参考复核基于 Codex 与 Claude 官方使用文档，未声称实机体验：
[Codex/ChatGPT 项目与对话](https://learn.chatgpt.com/docs/projects)、[队列与 steer 设置](https://learn.chatgpt.com/docs/reference/settings)、[批准与沙箱](https://learn.chatgpt.com/docs/agent-approvals-security)、[Claude Desktop 导航](https://academy.claude.com/tutorials/navigating-the-claude-desktop-app)、[Claude 连接器](https://support.claude.com/en/articles/11176164-use-connectors-to-extend-claude-s-capabilities)。借鉴聊天入口、连接设置和动作许可分层；本产品的设备执行权威仍在 Rust。

## 资源上下文与系统外观

软件/工具目录按已提供类型分组，点击卡片仅浏览详情；“查看并确认”仍是独立的人工操作入口。资源详情中的“询问 AI”先显示入口、分类、名称和详情，再要求选择现有会话或新建。首次发送才创建新会话；预览和移除资源信息后显式发送。每会话保留一个待发送资源附件，仅包含后台公开名称、已提供的软件版本及展示信息修订；分类、说明、目录版本或能力判定缺失时明确标记，不推断。任务编号、执行账号、任务记录、秘密和设备信息不附带。后台列表或修订变化后需移除附件或从最新信息重新选择。关闭面板后可在 AI 页面继续同一会话；身份切换清空旧主体的草稿。

1100×760 和窄窗口使用右侧抽屉；1440×960 的完整桌面内容区按需并排。主页面与资源面板使用不同构图，但复用同一个 Assistant 和输入组件。本人确认、执行授权和任务查询仍使用既有后台任务路径；上下文询问不执行任务。浏览器测试样本单独装配，正式界面不以样本替代后台返回。

外观使用同一启用与实色回退策略：macOS 采用 Sidebar 原生材质，Windows 11 build 22621 及以上采用公开 DWM Mica，其余宿主实色。正文、输入和模态抽屉始终实色；减少透明度、减少动态、高对比、系统设置读取失败或材质调用失败均回退实色。窗口创建、获得焦点及系统主题变化立即核对，窗口存活期间每两秒刷新；材质失败不会阻止工作区使用。

macOS 透明 WebView 启用了 Tauri 的 `macos-private-api`，该路径影响 Mac App Store 接受；当前企业桌面候选不承诺商店发布、签名或公证。`pnpm check:desktop-native` 仍限定 macOS arm64，分别记录真实 WebView 与模型/执行 fixture 的证据。Windows 交叉 `cargo check` 仅证明类型与编译接缝；真实材质、辅助设置、DPI、拖拽和最大化必须由 Windows 原生环境验证，未运行时不得标记通过。
