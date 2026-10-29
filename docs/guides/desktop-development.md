# 桌面开发与验证

桌面生产入口通过认证 IPC 消费 `apps/agent-service`，不持有执行 SQLite 或 S1 runner。React 桌面和 AI MCP 共享后台任务引用；系统服务持有唯一执行 intent、恢复和结果送达状态。独立 Node AI Host 与 provider worker 只承接会话，不签出企业批准。后台企业任务授权和来源由服务端决定；客户端校验签名、材料、设备注册及 OS 会话。

生产界面采用 React / Ant Design，保留 Tauri。布局与交互以 [PRD 的 UI/UX 基准](../product/rss-mdm-agent-prd.md#桌面-uiux-基准)及固定 v4 原型为准。
浏览器检查使用 `pnpm check:assistant`；额外的 macOS WKWebView 检查使用显式 `pnpm check:desktop-native --visual`，真实受控服务旅程使用 `--controlled-service`。普通本地 CI 不启动原生视觉验收。

## 启动和运行包

使用根 manifest 固定的 Node、pnpm 与 `rust-toolchain.toml`。所有命令在本仓运行。

```sh
pnpm install --frozen-lockfile
cp env.example .env      # 填写实际服务地址、租户 UUID 和组织名称
pnpm dev --fixture       # 持续运行的真实原生 fixture；无需生产服务、组织配置或模型
pnpm dev                 # 自动准备/复用开发 AI Host 后启动桌面
pnpm desktop:build       # 自动打包 Node/依赖、stage、构建 .app
```

生产 Vite 构建根据实际输出模块（含延迟 chunk）拒绝 Vue / Reka、测试装配及原型资源，生成 `dist/frontend-closure.json`，可用 `node scripts/frontend-closure.mjs --check apps/desktop/dist` 复核输出是否被增加、删除或替换。`pnpm desktop:build` 结束后写入 `.local-ci-runs/desktop-release-static.json`，绑定源码、前端文件、原生程序及 release runtime 摘要；macOS 同时核验应用代码签名。该回执不启动应用，不证明原生 UI、安装或双平台验收。

真实模式开发与打包都要求仓库根 `.env`，无论命令从哪里运行；缺失或字段为空时立即停止。`RSS_MDM_ORIGIN` 填写真实 HTTPS origin，`RSS_MDM_TENANT_ID` 填写管理员提供的非零 UUID，`RSS_MDM_ORGANIZATION_LABEL` 填写组织名称。模板的 `https://mdm.example` 为占位地址，构建和手动组织保存都会在网络请求前拒绝 `.example` 域名。实际 `.env` 被 Git 忽略；不在其中配置账号密码、token 或设备注册秘密。

组织私有 CA 统一通过根 `.env` 的可选 `RSS_MDM_CA_FILE` 配置，填写单个 PEM CA 公共证书的绝对路径；留空使用默认信任。构建时读取、校验并嵌入公共证书，不嵌入路径或私钥；桌面发布后不读取执行服务配置或外部 CA 文件。只有构建默认组织的 origin 与 tenant 同时匹配时使用该 CA，其它组织与个人 AI 不继承。保留证书链、有效期和主机名验证，不修改系统证书库。更新 CA 后重新构建桌面，并从同一 `.env` 生成 Agent 配置，步骤见[服务指南](local-service-lab.md)。

这三个字段在构建时校验并编译进桌面原生程序，开发与发布使用同一条路径。修改 `.env` 后重新运行 `pnpm dev` 或 `pnpm desktop:build`；地址变化不重建 AI Host runtime。发布应用从 Finder 或开始菜单启动时不读取外部 env、工作目录配置或运行时环境变量；普通 Cargo 编译和单元测试可以没有部署配置，实际应用启动要求有效内置连接。原始 `.env` 不进入应用资源或前端 bundle。

账户设置预选内置默认组织并显示实际地址与租户，但不自动登录。默认组织不写入本机组织连接文件；用户可以添加、选择其它组织。与默认组织相同的地址和租户只显示一次，默认名称由构建配置决定。更换构建默认连接不会遗留旧内置记录，也不清理用户主动保存的连接或历史。系统设备服务仍使用管理员配置的 `execution.json`，需独立核对后端、租户、签名信任和注册状态；本入口不自动修改服务或重新注册设备。

分步诊断可单独执行 `pnpm bundle:ai-host` 和 `pnpm stage:desktop-runtime`。`pnpm dev` 使用独立的 `.local-ci-runs/ai-host-dev-runtime` 和 development manifest，允许未提交源码；首次启动自动构建，后续按 Host、adapter、contract、相关 workspace 包、Rust execution schema 与绑定检查、lock、固定 Node 和打包脚本的内容摘要判断是否重建。普通 UI 修改不触发重建。每次启动校验运行包完整性、manifest 与本轮源码摘要一致性及真实 CLI 生命周期，准备失败会非零退出，不启动 Tauri。修改 Host 后重新运行 `pnpm dev`。终端断开（SIGHUP）、中断（SIGINT）与停止（SIGTERM）均转发到独立开发进程组并有界清理。

高级诊断可用 `RSS_AI_HOST_RUNTIME=/absolute/verified/ai-host-runtime pnpm dev` 显式选择并验证运行包。打包直接复用源 lock 的外部依赖图，并绑定本地 tarball 的完整性摘要；冻结安装优先使用缓存，缺失的包或策略检查 metadata 可联网补齐，不要求不同 pnpm 版本共享缓存格式，也不重新解析依赖。缺依赖先运行 `pnpm install --frozen-lockfile`；构建失败查看命令输出；运行包损坏时删除开发 runtime 目录后重试。准备取消会等待本轮进程组清理确认后释放锁；权限不足或回收超时将非零退出并保留锁与进程诊断，不能视为已清理。确认本轮进程及后代均已退出后删除 `.cache/desktop-dev.lock`。开发包不进入发布 stage；release 忽略该 override，并校验实际运行包完整性。

普通 Cargo/schema 检查使用基础 Tauri 配置，不依赖运行包；macOS 发布构建显式合并 `tauri.bundle.conf.json`，Windows 显式合并 `tauri.runtime.windows.conf.json`。自动加载的 `tauri.windows.conf.json` 仅保留平台窗口设置，不要求尚未生成的运行资源，避免 schema/验收辅助程序与运行包生成形成循环依赖。打包脚本先校验固定 Node archive、锁定部署依赖、真实 SDK 生命周期，再将通过的当前候选复制到被忽略的 resources 目录。macOS bundle 通过 `bundle.macOS.files` 整目录复制 runtime，以保留 pnpm 依赖符号链接；普通 resources 文件枚举会漏掉这些链接，不能用于该 runtime。发布应用只从自身资源目录启动 AI Host。缺失或不可用的 AI 不影响 Rust 任务读取，也不会降级为虚构对话。

桌面用户状态、Host 路径配置和 AI 库保存在应用数据目录 `desktop`；执行 journal 只属于系统服务。已有测试用户数据原样保留，不作为当前界面的登录入口，也不自动迁移。个人连接从“用户信息 → AI 与连接”保存并单独验证；配置与凭据不改变设备授权。旧 `s1/client.json` 不读取、不迁移。AI 会话协议和数据库格式升级后明确拒绝旧格式，保留旧文件，不自动迁移或重建；只有显式 `native-e2e` 测试构建支持 `--test-data-dir <新的绝对目录>`，旧历史不能直接导入。连接来源、版本和认证约束见 [AI Host](../../apps/ai-host/README.md)。

在“AI 与连接”的连接卡片中设置新会话默认；已有会话的选择不随默认值变化。“会话选项”抽屉为当前会话选择连接、明确让下一条输入开始新上下文，或请求恢复原模型上下文。切换连接等待已接收队列完成，不重发输入。带入历史默认关闭：先明确开始新上下文或切换连接，再选择最近 N 轮或全部已完成文本、核对冻结预览，再确认随下一条输入带入一次；内容或连接修订变化后须重新预览或移除。读取原会话仅核对视图，与恢复模型上下文分别操作。恢复能力未知、不可恢复、已退休或原请求未确认时，显示具体约束并保持只读。

身份变化会销毁旧工作区，原生 IPC 核对捕获的 generation，迟到返回不能进入新主体视图。原有设备任务的发起 actor 不变；模型取消请求与取消确认分别记录，未确认时保留未知结果。同一主体重新连接只恢复其原会话和已加载历史，不重发设备动作。

## 企业登录与本机工作区

启动时先读取本机会话；没有有效工作区时，可选择“使用本机工作区”或“登录企业账号”。进入工作区后，点击用户信息，进入应用设置的“账号与组织”管理组织连接和企业登录。企业登录弹层在同一表单填写账号与密码；本机工作区不代表企业认证，个人 AI 和设备操作仍分别受连接状态、后台任务与当前主体授权约束。当前 React 界面不提供按名称创建或切换测试用户的入口。

企业登录先添加组织名称、HTTPS 服务 origin 与租户 UUID，再选择组织并填写本地账号。登录表单显示当前组织与实际服务；密码从页面经固定的原生登录命令一次提交，由 Rust 发送到该组织服务，不保存、不进入诊断或日志。提交、取消、切换组织及离开登录页会清空密码。仅接受系统信任的 HTTPS，不跟随重定向，不接受 URL 中的凭据、路径或查询。组织配置不构成授权；切换连接必须重新登录，随后分别查询 Identity 会话与 rss-mdm 当前业务授权，核对实例/租户/主体，并要求至少一个有效业务 grant。已注册设备登录需由唯一系统服务确认 current-user 绑定后才能进入工作区；响应丢失时重试原操作，冲突或原会话证明丢失时可明确选择清理后重新登录。即时本机撤权和远端关系清理分别确认。确定未安装或尚未注册的设备仍可独立使用企业会话，服务不可用或状态不明不作该降级。企业执行走已注册系统服务，企业批准不在客户端重新签发；本地 AI 的产品归属不随 provider 账号/API key 变化。

本轮使用产品已提供的 Identity v2 本地密码认证与 rss-mdm 业务授权 v1 HTTP 接口。系统浏览器 OIDC 到原生客户端的凭据交接尚无公开契约，因此此入口不支持 OIDC-only 账号。组织连接列表保存在本机；它不是服务端组织成员目录，也不自动发现或认领组织。

企业 cookie/CSRF 只保存在原生进程内，重启必须重新登录，不支持离线授权缓存。AI 连接、发送、接收及连接保存前在线复核；原生进程每 2 秒发起被动复核，每个 HTTP 请求最多 8 秒（依次核对认证和授权），不延长 idle 期限。撤销在下一次成功核查时发现，断网/核查失败即撤销本地上下文；不宣称服务端推送式瞬时撤销。已知过期时间也在原生 IPC 与 Host caller 准入处检查。退出或切换先关闭旧视图、取消旧模型工作；清理失败停止 Host，新账号不能复用旧通道。服务端注销无法确认时本地仍退出，并提示通过企业账户管理撤销服务端会话。已登记测试设备任务保留原 actor 和运行事实，不因账户切换伪造取消。

企业 AI 历史按服务地址与实例绑定的 authority、租户、主体三个标识归属；显示名、用户名、email 与 provider 凭据均不能认领旧数据。现有测试数据原地保留，只能回到原测试用户读取。本轮不迁移、不丢弃，也不提供跨账户导入/导出；未来迁移必须通过单独的显式选择与授权流程，不能直接修改用户名或数据库 owner。访客退出后再次选择“不登录使用”仍回到同一本机访客数据。

## 执行动作、确认和恢复

系统服务的 Rust journal 持有唯一执行状态。桌面展示后台 Offer；人工按 task、attempt、revision 和原 request 请求执行，是否展示确认由前端决定。MCP `execution_tasks` 读取同一批任务，`execution_execute` 发起精确任务，二级风险的 AI 动作才等待桌面专用确认。没有本地目录转计划或任意脚本入口。提交超时后查询原 request，不能换 ID 重派。

人工动作在已有权限内执行，后端不强制产品确认。AI 动作由 rss-mdm 后台可信策略按真实内容和运行上下文分为 0/1/2/3：0/1 在允许规则内直接执行，2 由当前有执行权限的用户确认，3 与未知默认阻止。明确禁止或无权限不能通过确认放行；AI 经用户确认后仍保留 AI 来源。Policy 保持独立非交互授权。AI 执行不再增加通用 provider 许可；二级风险只保留一次具体动作确认，模型回答不能代替它。OS 授权独立判定。当前远程 V1 没有风险签名字段，正式装配把 AI 风险视为未知并拒绝执行；后台策略、签名契约与最终接线由 #2654 跟踪。

桌面显示动作、来源、目标、运行身份、风险等级与内容摘要。确认有效期最长一分钟且不超过原执行有效期，等待不消耗进程运行预算。确认、取消及响应丢失均保留原请求身份，不重新创建任务；重启和 Unknown 只核实原 attempt。请求列表按原请求 ID 分页，每页最多 128 条，所选任务独立读取。

正式装配消费固定远程 Agent wire，本机 IPC、helper、部署格式和 SQLite 的版本由各自代码持有；旧格式明确拒绝并保持原文件，不迁移、清空或新建 journal 绕过未决任务。生产启动失败直接显示诊断，不引导切入测试目录。测试专用构建和 S1 样本不代表生产结果。监听前完整核验固定候选；每次 IPC 再核验实际 OS 对等主体、会话与受保护映像。桌面执行端口串行使用原生通道，已有 16 个准入名额覆盖等待及活动读取；对话流式更新不重复启动同一任务读取。

AI 来源由 Host metadata 与 Rust 当前用户绑定核验，不能通过工具参数改为 Human。UI 确认原请求时单独核验当前用户权限，执行来源保持不变。

关闭窗口销毁视图并分离 ACP 连接，桌面 runtime 和模型工作继续。显示窗口/应用 Reopen 创建新视图并读取持久状态。明确“退出”有界关闭桌面 Node/worker/MCP 和 runtime；系统服务由系统管理，不随桌面退出停止或隐式取消设备任务。服务异常退出后恢复 journal 只核实已有尝试；runner 历史丢失保留 Unknown。

Host 在发送工具操作前原子保存 delivery intent；回复丢失后按原业务 ID 和精确 plan 核实。`outcomeUnknown` 不生成完成回执；仅明确未提交才允许发送。交付可在 AI 本轮结束、provider 不可恢复或 Host 重启后继续；它不能改变模型命令或伪造 provider 结果。Rust 回执引用在 Host 落盘后才能确认。当前 MCP 查询不消费 Rust 结果日志，业务证据由 Rust 保留。

## 边界和验证

`self-service/native.ts`、`assistant/native.ts`、`settings/native.ts` 和 `test-users.ts` 只调用各自固定字面量 IPC。WebView 无网络、文件、进程、已存凭据读取或 SDK 入口；登录密码仅通过固定原生命令提交。capability 只授权本地 main 窗口。原生进程、socket 和 SQLite 只在 Rust composition 层。CSP、导航拒绝、IPC ACL、源码 AST 守卫分别验证，源码守卫不是 OS 沙箱证明。fixture 自己持有测试 Human/OS session 构造，源码守卫拒绝 fixture 反向依赖 composition。

Rust 命令、task details 和 MCP schema 生成前端/模型声明，无手写第二份 wire：

```sh
node scripts/check-self-service-bindings.mjs --write
node scripts/check-execution-bindings.mjs --write
cargo test -p rss-mdm-desktop --test composition --locked
pnpm test:ai-host
pnpm check:boundaries
make ci CI_BASE=origin/develop # 按影响范围；ci-full 强制全量，ci-plan 查看计划
```

macOS 的正式 `make ci` / `make ci-full` 对整个 Node 与 Rust 检查过程取得 worktree 独占租约，并将 Cargo `target` 放进默认四槽的本机缓存池；同一 worktree 优先复用槽位，槽位换属时清理旧产物。`AGENT_TARGET_POOL_N` 可设置 1–32 个槽，`AGENT_TARGET_POOL_ROOT` 可指定专用空目录或已标记的池目录。分配器短时竞争最多等待 2 秒；槽满或同一 worktree 已有受管运行时立即失败，原运行结束后重试。`make ci-plan` 不占槽；Windows 仍使用原入口。受管运行拒绝外部 `CARGO_TARGET_DIR`、`CARGO_BUILD_TARGET_DIR` 和自定义 rustc wrapper。

CI 在本轮准备阶段构建选中的包与 Rust helper，后续检查复用本轮产物；各包的 `build` 保持自身编译职责。独立测试使用对应 `pnpm test:ai-*` 或 `pnpm check:boundaries`，打包使用 `pnpm bundle:ai-host`，入口会准备自身所需依赖。消费 Rust helper/runtime 的测试文件直接执行时，需要父进程提供本轮产物路径；输入变化、缺失或损坏产物会阻断复用，不读取旧 CI 回执。临时准备目录随 owning run 结束清理，发布与开发 runtime 仍使用既有目录和 manifest。

定向 Rust 验证可用 `python3 scripts/build-run.py -- cargo test --locked -p <包名>` 复用同一槽位机制。直接运行 `cargo` 使用仓内 `target`；直接运行 `pnpm` 或 `cargo` 不取得 CI 的 worktree 租约，避免与同一 checkout 的正式 CI 同时修改仓内 Node 产物或运行回执。旧仓内 `target` 不自动迁移或删除。

登录界面的扫码与本机 IM／零信任应用快捷登录仅在显式 fixture 装配中展示；应用名称和账号状态为测试样本，不探测或授权真实第三方软件。生产装配不挂载这些入口。

浏览器开发使用显式 fixture 装配；样本不作为真实桌面/AI 验收。真实模型与原生窗口验收仅覆盖 macOS arm64、manifest 固定的 Codex；不要求 Windows/Linux 或三个引擎完成同一 E2E。受控服务验收覆盖 macOS 固定候选的真实执行与效果；开发机不执行 Windows 编译、测试、打包或真机验收，已完成实现项按[验证规则](../rules/verification-scope.md#开发机平台验证与实现项关闭)关闭并记录 Windows 未验证。独立平台安全矩阵、正式签名、公证、版本升级及 T3 企业身份按对应 owner 的有效范围另行验收。

来源：Tauri `crates/tauri/src/app.rs` / `webview/webview_window.rs` @ 2.11.2；runtime-wry `src/lib.rs` @ 2.11.4（最后窗口销毁与 ExitRequested）；rmcp `src/model/meta.rs` @ 3.4.0（request metadata）；MCP TypeScript SDK `client/index.ts` / `shared/stdio.ts` @ 1.30.0。

自动回归由本地 CI 的单元、协议、SQLite、adapter 和浏览器检查承担，不要求系统授权弹窗。`pnpm check:desktop-native --visual` 是额外的原生视觉验收，从仓库根启动 `pnpm dev --fixture`，加载产品 main 和真实 WKWebView。它复用既有 assistant fixture 装配同一个产品 App：资源快照和 AI Host 为显式测试来源，任务阶段、过程与效果取自 Rust 生成的测试投影，AI 关联为测试绑定。它不连接云端模型、不执行设备操作，不证明生产执行授权或效果。

该路径限定 macOS arm64，需要已解锁桌面及 System Events 辅助功能权限；环境不满足直接失败。检查浅深主题、1100×760 / 1440×960 / 480×400、会话与资源布局、任务抽屉和关闭重开；系统主题在结束后恢复。无实际入口或环境的缩放、其它显示比例及 Windows 项目明确标记未验证。

真实执行旅程需要显式 `--controlled-service`，使用受保护固定候选、真实认证 IPC 和服务 journal；普通开发启动不会自动安装服务。调试构建的真实 AI 路径使用隔离 macOS 文件钥匙串，退出后删除并核对默认钥匙串与搜索列表未改变；fixture 路径不创建钥匙串。正式签名应用的钥匙串授权另行验收。

驱动使用固定 `webdriverio@9.32.0` 与 `tauri-plugin-wdio-webdriver@1.4.0` 的标准 WebDriver 接口。`native-e2e` 只允许调试构建，并要求显式 nonce、动态 loopback 端口和隔离 `--test-data-dir`；release 携带该 feature 会编译失败。脚本核对主进程与监听端口归属；固定的 [本地上游补丁](../../vendor/tauri-plugin-wdio-webdriver/NOTICE.md) 对每个请求校验运行凭据，缺失或错误凭据一律拒绝，启动日志仅记录凭据摘要。脚本不调用 WebView 内部 IPC、不替换业务回复。视觉模式的关闭/重开通过 WebDriver 原生 `window.close()` 与应用“显示窗口”菜单验证；开发窗口的标题栏关闭按钮未向 AX 暴露，物理按钮点击单独标为未验证。业务交互使用可见产品按钮；应用菜单操作由 macOS System Events 发出。开发构建验收期间保持源码不变；固定候选验收允许源码变化。不要并发启动同一 worktree 的开发服务。

`.local-ci-runs/desktop-native.json` 记录实际模式、当前工具运行环境、运行包摘要、进程归属与完成项；固定候选保留冻结时来源，开发构建路径另记源码与锁文件摘要，截图和脱敏日志在同目录。旅程结果、wrapper 退出和清理确认分别记录；wrapper 非零退出或清理未确认时，即使旅程断言通过，总结果仍失败。失败回执不能解释为通过。fixture 模式证明产品 WebView 与显式样本装配；controlled-service 模式分别保留真实服务 journal、后台回执及独立设备效果证据。本地模型响应仍是受控样本，不证明真实模型能力。

发布资源验收使用 `pnpm bundle:ai-host && pnpm check:desktop-bundle`。入口构建带隔离应用标识和测试内置连接的实际 release `.app`，从无测试参数的生产 main 启动；核对 Host、内置组织不落盘及运行时配置不能覆盖默认连接，不主动登录。禁用 runtime override，不包含原生驱动，核验完整 runtime 树与 Native health 握手；结果写入 `.local-ci-runs/desktop-bundle.json`，绑定源码、锁文件和 runtime manifest。此 smoke 只证明启动与资源定位，使用隔离进程组清理，优雅退出由日常原生验收验证。

真实模型仍单独运行 `pnpm smoke:codex`，按 [AI Host](../../apps/ai-host/README.md) 的显式配置入口选择认证、端点与模型；它保留跨轮上下文证据，不由本地协议夹具替代。真实模型、原生 WebView 和 release 资源启动是三份不同范围的回执。

来源：WebdriverIO `desktop-mobile/packages/tauri-plugin-webdriver` @ `wdio-tauri-service@v1.4.0` (`aef40049a9c566e72de4ffd08e08197ff32386ed`)；[Tauri WebDriver 指南](https://v2.tauri.app/develop/tests/webdriver/)。

## 设置、诊断和 AI 恢复

设置入口始终可达，未选择用户或 AI Host 不可用时仍可查看诊断与关于。账户区域区分网络不可用、限流、账号/授权失效及服务响应不兼容；成功进入新的账户后清除旧原因。首次路径是选择账户入口、保存配置、测试连接、发送首条消息；“开始对话”与“稍后配置”都只进入本地空白页，首条消息才创建产品会话。常规与通知仅说明已有行为，不提供无底层服务的开关。

Native 长期持有用户、设备执行 IPC 消费端口和应用主密钥访问 owner；Host 进程可以单独替换。启动前核验关键文件、平台和契约版本，配置、存储和恢复完成后的私有 health 应答才表示 ready。运行包不匹配直接拒绝；release 构建忽略开发 override。关闭/回收旧 Host 后才启动新代，worker fence 未解决时禁止重复 worker。

Native 在每次启动前按打包端同一规则重算文件字节、权限和符号链接图摘要；`.app` 候选的预期摘要由 stage 后的 Native 构建绑定，改写资源目录中的 manifest 不能替换它。显式开发 override 校验其 manifest 与实际树一致，信任由开发者选择该路径建立。health 超时或不合法时先撤销 control/MCP，再有界停止并回收进程；回收未知保留原 owner 阻断新代。

“重新连接”只建立视图通道并核对历史，不重启 Host，不重发请求。“重启 AI Host”有明确提示，模型请求可能中断，设备任务继续；它不调用设备服务关闭，也不记录虚假的任务完成或取消。Host 不可用时仅显示当前用户已加载的只读历史；未加载历史在恢复后按原接口分页读取，切用户清空全部旧视图。不存在 Native SQLite 历史旁路。

连接操作区分配置、认证、能力、限额、拒绝、取消和未确认结果。仅明确认证错误提示更新凭据；普通失败不伪装为认证失效。普通对话运行真实文本探针，受控用途还要求所选模型调用验证专用无副作用工具；探针不接设备执行服务。配置先独立保存为未验证修订；测试及进程退出确认后才追加可用修订，失败保留已保存配置和凭据。Codex 禁止模型 fallback，并核对返回模型；已有配置的默认模型由官方工具解析。

诊断只包含闭集阶段/错误码、时间、运行版本和资源来源类别，最多保留 64 条故障记录。通过原生保存对话框导出，不包含凭据、原始错误、端点、个人路径、对话或数据库内容。缺包按开发/发布来源分别提示构建或重装，旧进程回收未确认则保留阻断。

私有通信遵循唯一生成契约，无法归属到具体请求的错误不能被猜测为认证失败。

设计参考：Microsoft [设置指南](https://learn.microsoft.com/en-us/windows/apps/design/app-settings/guidelines-for-app-settings) 与 [WinUI Gallery SettingsPage.xaml](https://github.com/microsoft/WinUI-Gallery/blob/main/WinUIGallery/Pages/SettingsPage.xaml)。桌面使用 React / Ant Design / Tauri。

## 助手开发

```sh
pnpm dev:assistant-fixture
pnpm check:assistant
```

打开 fixture 打印的 loopback 地址；它加载实际产品 App，使用同进程 FakeHost、实际 Rust/SQLite S1 样本，不连接真实模型或执行 OS 操作。macOS 默认使用系统 Chrome，其它环境通过 `AI_BROWSER_PATH` 指定 Chromium。测试失败不会降级为静态渲染。

手工体验任务页时，打开 fixture 地址的 `?desktop&scene=tasks`，从“任务中心”读取样本原任务及节点、取消意图和核实事实；样本请求不代表真实设备执行。

“首页”提供电脑模型、当前加载的软件目录以及设置、软件、工具和任务入口。电脑模型来自用户提供的 A 款设计稿，仅作示意；计算机名、IP 地址和系统登录用户在电脑下方以图标与值的紧凑信息行展示，通过已认证的本机服务读取，失败或字段缺失时显示占位符，不再显示整段不可用提示。进入首页及停留期间每 30 秒刷新，多地址在提示中展开；只显示当前登录会话的系统用户名，不使用企业登录名或 AI 账户替代。开发 fixture 使用 Studio Mac、文档保留地址 192.0.2.24 与 studio 样本，不调用生产设备接口；软件目录不代表已安装清单。电脑只随内容区宽度等比例缩放，单独调整窗口高度不改变其尺寸。右侧电脑助手与当前会话共用草稿，点击箭头进入 AI 页面后再显式发送；已有草稿时提问引导不会覆盖它。

有效主体进入后默认打开主页。左上 RSS 控件按“无 → 小 → 宽 → 小 → 无”切换侧栏，⌘ / Ctrl B 隐藏或恢复；无侧栏时从顶部模块及二级导航访问全部入口。账号与组织、AI 与连接、个人 Passkey、运行与诊断等设置通过用户信息直接进入，二级设置在侧栏底部向上展开，与系统设置分开。任务中心是独立一级模块，任务详情使用抽屉。导航切换保留原会话、草稿与各会话阅读位置；身份变化销毁旧工作区及其缓存。同用户重连保留已加载历史和未知命令身份，在权威恢复完成前只读，不自动提交。

个人连接从“用户信息 → AI 与连接”添加或编辑，先保存配置，再验证已保存版本。保存不发送模型请求；验证可能访问提供方，失败仍可编辑。AI 页面不增加顶部菜单；新会话与历史会话在二级导航，点击历史即打开固定清单，无 pin 操作。历史只搜索已读取记录，可分页读取。首次创建和发送分别使用稳定身份；未知结果先核对原请求，创建期间编辑草稿或切换会话不会后台发送。运行中可编辑并排入下一轮；提供方支持时允许转向当前回复。新密钥只存在于输入表单，保存、端点或凭据类型变化以及离开设置时清空，不回显已存凭据。

助手回答支持列表、代码及表格排版，代码块可复制；链接仅显示文字与可复制地址，图片不自动加载。复制不可用时按提示选中文本手动复制。长内容默认展开，可主动收起。

设备任务卡与其详情共用同一次授权读取；“刷新状态”核对原请求，失败保留上次记录并提示，不表示实时进度。停止回复只作用于模型，取消设备任务进入原有任务入口。

AI 文本不能覆盖设备任务事实；执行详情来自 Rust 的授权读取。普通回答与 A2UI 动作均不能签发批准。未知接纳重试保留原命令和期限，回执不等于模型终态。过期、旧 generation 和删除卡片禁止继续提交；渲染失败保留只读内容。

`execution_execute` 与三个读工具在 Host 投递前不请求通用许可；`execution_task_control` 保留“允许一次 / 拒绝一次”。相同待决提案共用一次询问，已有持久投递恢复不重问。许可过期、会话/命令结束或用户切换后不能写新执行意图。取消许可仅允许 AI 发起取消请求；Rust 在实际执行前验证后台授权和按风险要求的具体动作确认。时间线的设备卡只从 Host 投递事件定位，再核对 Rust 返回的请求、会话和工具身份；模型文字不能生成可信设备卡。

交互参考复核基于 Codex 与 Claude 官方使用文档，未声称实机体验：
[Codex/ChatGPT 项目与对话](https://learn.chatgpt.com/docs/projects)、[队列与 steer 设置](https://learn.chatgpt.com/docs/reference/settings)、[批准与沙箱](https://learn.chatgpt.com/docs/agent-approvals-security)、[Claude Desktop 导航](https://academy.claude.com/tutorials/navigating-the-claude-desktop-app)、[Claude 连接器](https://support.claude.com/en/articles/11176164-use-connectors-to-extend-claude-s-capabilities)。借鉴聊天入口、连接设置和动作许可分层；本产品的设备执行权威仍在 Rust。

## 软件管理

正式软件页读取现有 `software.installations`、`software.receipts` 和 `software.packages` 数据投影；进入页面、搜索及计数不会执行采集。来源页与安装项分别分页，显示采集时间、过期、枚举质量和已加载范围。需要新事实时，在“数据来源与刷新”显式提交查询；回执丢失时查询原刷新请求。

“已安装”包含当前用户可见的应用、收据和软件包；安装类型图标不冒充软件自身图标。收据不代表 GUI 应用。详情显示实际用户/设备 scope、原生标识和各动作不可用原因。“可更新”仅显示受保护配方中来源、适用源版本和原生版本规则均满足的固定候选；未知版本顺序不会作为更新计数，也不会按名称关联企业发布与安装记录。

软件中心“发现”与五类目录消费正式 Policy 自助目录；“已安装”与“更新”读取本机观察。发布目录不会被解释为已安装。查看、准备与确认提交分开，提交时复核精确 Policy、版本、主体和最终参数；安装与企业更新/卸载仍走原签名任务。目录刷新与分页不会自动执行。

本地更新/卸载先在唯一执行 journal 保留准备；用户查看范围、版本和原始来源后确认原请求。设备级卸载由已安装的系统服务执行，无需当前用户提供管理员凭据；用户级动作和 GUI 启动通过本人原登录 helper 执行。打开按钮及其 Enter 操作共用同一请求，准备/派发期间不会重复启动。AI 只能准备、查询和取消本地操作，不能提交；可信软件卡从原请求查询服务，确认保留原 AI 配置、会话和调用来源。

原操作从任务中心、软件详情及 AI 卡片查询同一 journal，不依赖浏览器保存执行参数。失去回执只查询原 UUID；取消、进程退出、全部停止、重启等待和原生检测分别展示。只有精确原安装记录的独立缺失检测才能改变该安装快照；退出码不能改写安装状态，重新采集后以新的观察为准。

## 资源上下文与系统外观

页面助手从当前页面附加受限展示投影，并与 AI 页面共享当前会话及草稿。附加内容只来自明确的 page projection，包括原始来源、修订和观察时间，不从 DOM、未提交表单或秘密字段取值。发送前可移除上下文；过期或来源变更须重新读取。上下文不是执行授权，询问不自动运行任务，身份变化清空旧主体的附件和草稿。

页面助手使用右侧面板，窄窗口按可用空间展示；在 AI 助手中继续仍是同一会话。本人确认、执行授权和原任务查询使用既有后台路径。浏览器样本单独装配，正式界面不以样本替代后台返回。

外观使用同一启用与实色回退策略：macOS 采用 Sidebar 原生材质，Windows 11 build 22621 及以上采用公开 DWM Mica，其余宿主实色。正文、输入和模态抽屉始终实色；减少透明度、减少动态、高对比、系统设置读取失败或材质调用失败均回退实色。窗口创建、获得焦点及系统主题变化立即核对，窗口存活期间每两秒刷新；材质失败不会阻止工作区使用。

macOS 透明 WebView 启用了 Tauri 的 `macos-private-api`，该路径影响 Mac App Store 接受；当前企业桌面候选不承诺商店发布、签名或公证。`pnpm check:desktop-native` 仍限定 macOS arm64，分别记录真实 WebView 与模型/执行 fixture 的证据。开发机不运行 Windows 交叉 `cargo check` 或 Windows 原生验收，也不以这些检查作为实现 issue 的关闭门槛。真实材质、辅助设置、DPI、拖拽和最大化可在目标 Windows 环境按需验证；未运行时标记未验证，Windows 支持声明须有对应实际证据。

## 显式 fixture 装配

`pnpm dev --fixture` 打开 Tauri main/WKWebView 并保持运行。默认展示完整生产 React 页面与显式样本 ports；浏览器 fixture 的无参数地址仍为基础控件验收页，右下“隔离样本与验证工具”选择场景。`scene=tasks` 提供原任务样本，`scene=loading` 等待测试端显式释放发布目录读取。所有样本都不代表认证、模型或 OS 执行证明。

此模式复用浏览器 `pnpm dev:assistant-fixture` 的数据/服务 owner，不读取生产账户、凭据、服务或执行 journal，不准备独立 AI Host，也不要求自动视觉验收的 WebDriver/System Events 权限。正式构建不能启用 fixture，真实服务异常不会切换为样本。

`pnpm check:desktop-native --visual` 自动使用同一 fixture 装配；`pnpm check:desktop-native --controlled-service` 明确请求固定候选的真实服务旅程与受控安装。两者回执分别记录 fixture 和真实执行事实。真实模式的候选、注册、可信配置和协议必须匹配；部署/刷新及管理员操作见[服务指南](local-service-lab.md#显式开发刷新)。

企业脚本与工具读取当前登录员工和设备共同获准的 Policy 自助目录。填写 Resource 的 Input 参数后，人工入口会展示配置 revision、执行版本、资源版本和最终参数；`allowAi=false` 只关闭 AI 入口。
AI 使用实际注册的 Policy 工具，风险 2 的原请求需要本人在桌面卡片确认一次。确认后策略变更、退出登录或期限届满会阻止启动。
提交回执丢失时查询原请求 UUID；未知结果不允许创建新请求重试。脚本结果保留原始采集时间、完整度、schema/预算判断与输出引用，软件检测只返回受限原生 observation。

## 基础控件展示

`pnpm dev:assistant-fixture` 启动后打开打印的地址，可操作 React / Ant Design
主题、表单、菜单、软件封面、状态、消息和详情抽屉。`pnpm dev --fixture` 在真实
Tauri main/WKWebView 中打开同一显式开发装配。这些入口不进入生产导航。

`pnpm check:desktop-native --visual` 验证 React 组件、业务页面、任务和 A2UI 的显式 fixture，以及真实 WKWebView 键盘、主题和窗口生命周期；不执行设备任务。真实服务验收使用独立的 `--controlled-service` 模式。浏览器中的平台预览不证明 Windows 原生行为。
实际 OS/build、浏览器和原生检查结果随交付 PR 记录。

主页设备信息使用本地 IPC v15，桌面和本机服务必须作为同一候选更新；旧版本不降级或猜测字段。此变更不新增网络探测、设备写操作或 AI 设备信息附件。Windows 原生结果需在目标系统验证。

### 工具中心

工具中心可直接运行组织当前授权的脚本，参数与执行信息在同一表单展示。填写后点击“运行”，在原请求中查看状态和结果。
连接检查默认使用当前 RSS MDM 后端域名与端口，也可修改为单个域名/IP 和端口；结果分别报告 DNS 和 TCP，端口连通不代表业务恢复。
空间查看使用已有采集事实，显式刷新才采集新结果；清理须使用范围明确的授权工具。
诊断先选择 Host、服务、设备或执行摘要并预览，再选择保存位置；不可用或截断范围会保留在文件中。取消保存不会生成成功提示。


## 个人 Passkey Vault

“用户信息 → 个人 Passkey”属于当前 OS 用户，企业登录、Agent 服务和 AI Host 不可用时仍可进入。
默认使用系统认证解锁，可选设置独立个人口令；两者是独立解锁方式，不要求连续验证两次。
口令只在原生安全输入窗口输入，至少 12 个字符、最多 1024 个 UTF-8 字节；不裁剪输入。
修改口令可验证当前口令或系统身份；移除口令必须重新验证系统解锁。忘记口令仍可用系统身份
重新设置；系统保护丢失但口令有效时，可用口令恢复访问并重新建立系统保护。

本期仅本机保存，尚未同步。口令不是备份，丢失数据不能仅凭口令恢复；改口令不撤销已经保存
的历史密文或解密根。两种方式均不可用时 RSS 没有管理员恢复入口。系统自身的账号恢复能力
沿用平台行为，不承诺抵御系统管理员或内核失陷，也不宣称每把 Passkey 私钥硬件不可导出。

锁定、关闭窗口、退出、系统锁屏/注销/休眠会使当前原生会话失效；其他实例独立认证。
网站账号列表只在解锁后显示，不进入 AI 上下文或诊断。删除只影响本地条目，不删除网站公钥
或其他副本；没有备份时需到网站重新注册。损坏/未知格式保留原文件；明确重置后建立新空库，
旧密文保留不代表可恢复。企业退出、退管和卸载不主动清空个人 Vault。

macOS 桌面与 Credential Provider 扩展共用当前用户的 App Group 容器及 Data Protection Keychain；
只使用共享位置，不读取旧桌面私有目录，也不迁移或删除旧数据。共享权限不可用时个人 Vault 页面
明确报错，其他桌面能力仍可使用。Provider 在系统设置中由用户启用；每次请求独立解锁、选择账号
并确认，不继承桌面的解锁会话。锁定会撤销其他实例的待决请求与解锁会话。

系统凭据提示索引是可重建缓存，不是签名授权。页面显示系统启用状态与待修复状态；解锁后可点击
“修复凭据提示索引”，保存/删除后也会请求更新。失败保留待修复状态，不回滚已保存的凭据。
真正使用时重新检查 Vault、网站和账号，不能仅凭旧提示完成签名。

Windows 与 macOS Provider 分别适配各自正式系统入口；macOS 构建与后置验收边界见
[本地运行指南](local-service-lab.md#macos-passkey-provider2930)。
凭据允许未来备份，但尚未实际备份；#2931 另行实现同步、新设备授权及恢复，不将 OS 保护材料
上传服务器，不允许普通账号会话或企业管理员直接取得个人解密根。

## 任务中心

主导航“任务中心”统一显示正在进行、历史记录和管理活动。首页、搜索、软件/工具记录与 AI 卡片进入原请求；返回保留当前筛选和位置。切页或关闭聊天不取消设备工作。进程结束、效果核实、取消请求/终止证据和上传接收分别显示，未知结果先查询原请求，不创建替代尝试。

服务按当前主体和阶段声明可用控制。普通内容下载支持暂停/恢复，暂停意图在当前格式重启后保留；恢复仍须通过原授权及期限检查。共享材料可以继续为其他任务下载，暂停或取消一个父任务不影响其他消费者。Winget/Brew 原生源准备不提供 Range 暂停续传，界面显示原因。已知启动前失败的受支持 Policy 可明确创建关联的新请求，重新检查原策略版本并重新准入。

管理活动展示设备可见的脱敏来源事实及其关联，标明最近同步、保留起点和 coverage。离线缓存、部分页和未覆盖关联不代表即时全量历史；审计记录与上传 ACK 均不证明设备效果。任务投影仅提供参数类别摘要，处理原准备请求须进入独立人工交互。AI 不具有该人工确认入口。

本次替换要求桌面、系统服务与 AI Host 使用同一当前接口；旧 IPC 或旧数据库格式原样保留并拒绝，不自动迁移、清库、重新注册或替换 journal。
