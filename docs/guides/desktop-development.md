# 桌面开发与验证

当前桌面提供首页、软件中心、工具中心、参数表单、计划预览、请求任务与交互提示。页面持续显示“固定测试服务 · 无真实执行”。Rust 普通权限内存服务拥有测试请求，Vue 消费四个受限 IPC；不连接模型、特权后台、数据库或企业服务。浏览器仅显示由同一 Rust 服务生成的只读快照。

## 工程与启动

根 `Cargo.toml` 统一持有workspace、版本和Rust依赖，桌面成员为 `apps/desktop/src-tauri`；后续能力按任务加入 `crates/<name>`，不预建空包。
根pnpm workspace管理 `apps/*` 与 `packages/*`。Vue页面和草稿状态在 `apps/desktop/src`，原生窗口启动与打包配置在同应用的 `src-tauri`；通用组件位于 `packages/ui`。

使用 Node 24、pnpm 11.4.0 和 `rust-toolchain.toml` 固定的 Rust 1.96.0。先安装平台所需的[Tauri系统依赖](https://v2.tauri.app/start/prerequisites/)；macOS需要Apple命令行开发工具，Windows/Linux依赖以官方平台说明为准。

```sh
pnpm install --frozen-lockfile
pnpm dev                 # Tauri窗口；仅监听127.0.0.1:1420的开发服务
pnpm dev:web             # 同一页面的浏览器只读预览
pnpm build               # UI库、类型声明、应用前端
pnpm desktop:build       # 当前平台发布可执行程序，不生成安装器
make ci                 # 本仓完整检查，收集全部失败后返回非零
```

开发服务器端口占用时直接失败，不自动换端口。UI库源码变化后重新运行 `pnpm --filter @rss-mdm-agent/ui build`；桌面页面由Vite提供热更新。不持有第二份组件源码alias。

Rust build/test前需先 `pnpm build`，生产资源由Tauri嵌入。Cargo workspace根的 `target/release/rss-mdm-desktop`（Windows为 `.exe`）是发布可执行程序，可在无Vite服务时启动。启动失败向stderr保留底层错误链并非零退出；Windows GUI版本额外通过原生MessageBoxW显示相同诊断，不依赖WebView或控制台。Windows对话框的实际可见性需在Windows宿主验证，不能以macOS编译或单测代替。

## 组件消费

应用通过 `@rss-mdm-agent/ui` 公开入口导入组件/类型，并显式导入 `@rss-mdm-agent/ui/style.css`。Vue是peer dependency；CSS仅在 `.rss-ui` 下生效，不修改宿主body。导出 `AppShell`、`NavigationList`、`SplitPane`、`MessageStream`、`MessageComposer`、`StatusList`；类型见包内 `src/types.ts`。

- AppShell持有header/navigation/default/status slots；宿主给它明确高度。
- NavigationList只发出启用项id，由宿主更新activeId；原生button支持键盘操作。
- SplitPane为上下分栏，初始比例0.5、最小比例0.1；初始比例只在挂载时使用，动态minRatio会重新约束当前比例并更新ARIA。resize输出比例，宿主自行决定是否保存。
- MessageStream以带名称的polite log显示文本并通知追加/文本更新，reasoning默认折叠，不自动打开链接或解释终端控制序列。
- MessageComposer的modelValue由宿主持有；submit返回去除两端空白的文本，不自行清空。busy/disabled/折叠/空白阻止提交；canCancel独立表示宿主允许取消，disabled阻止取消。取消事件不证明任何任务已经终止。
- StatusList消费明确label/message/tone，稳定的polite/atomic status区域包裹原生列表，通知状态更新；不探测系统、认证或引擎。

`pnpm check:consumer` 先构建UI包，再打成归档，在仓外临时workspace离线安装并消费所有公开组件、类型和CSS，开启`noUncheckedSideEffectImports`并关闭`skipLibCheck`，不加载通配CSS声明以遮蔽归档缺失；检查生产构建及安全文本渲染；退出清理临时目录。首次先完成正常安装，以填充依赖缓存。

## 验收范围

`make ci` 包含frozen install、前端构建、类型、组件与样本测试、边界守卫及负例、独立归档消费、文档/diff、Cargo build/test/fmt/clippy；报告写入被忽略的 `.local-ci-runs/latest.json`，包含源码SHA、时间、平台及逐步结果；子进程exit status和signal分开保存，信号终止/无退出码有稳定失败原因。无相关Rust行为时不添加空测试。

原生窗口另做人工或系统辅助功能验收：启动→看到测试标识→浏览目录→填写参数→预览计划→提交或申请→任务交互→关闭退出。再停止开发服务器，启动发布程序重复验收，记录系统/架构和产物SHA。浏览器测试不代替此门。

C05只承诺实际记录的平台验证；Windows/Linux构建、安装签名、公证、升级、真实执行、模型接线与企业T3均不能由macOS样本推定通过。边界守卫扫描 UI 与桌面源码、Vue 模板及生产依赖。通用 UI 仍拒绝全部宿主能力；桌面唯一 native adapter 仅可调用四个字面量测试命令，继续拒绝网络、进程、插件与 HTML 注入。前端依赖和环境全局值采用allowlist，AST检查静态成员/别名、模板资源入口和原生表单提交；CSP逐directive/source检查，并独立突变CSP、capability、Cargo依赖和宿主注册。组件边界守卫是Medium检查，不是抵御任意恶意代码的操作系统沙箱。

宿主从唯一配置显式创建主窗口并挂接导航handler：macOS/Linux仅允许`tauri://localhost`，Windows仅允许当前配置对应的`http://tauri.localhost`，开发构建额外允许`http://127.0.0.1:1420`，拒绝凭据、非默认端口、其它origin及新窗口。CSP禁止外部资源、frame和表单提交，Tauri app manifest 显式声明四个测试命令，capability 仅授权本地 main 窗口的这四个命令；源码检查与实际运行限制分别承担职责。UI的`runtime.ts`聚合CSS，公开声明从无CSS导入的`src/index.ts`生成，归档消费者继续显式导入唯一`style.css`。

本地CI记录起止HEAD、CI_BASE对应merge-base及工作树状态；未提交源码或运行期间HEAD变动使验证失败。diff检查覆盖base到HEAD、暂存区和工作树。

## C15 自助服务与证据边界

传统页面完全替换旧的概览/消息演示。软件项目和工具共享目录 projection；所有参数通过 `service-catalog` runtime 校验，表单只编码输入，不补默认值。整数文本在 Rust 校验前保持原数字 token；秘密参数只接收精确引用，不解析秘密正文，摘要/任务/诊断不回显值。校验错误保留核心静态类别，不在 UI 猜测字段错误。

`SelfServicePort` 只有 snapshot、preview、submit、respond。Rust 的单一 RequestRecord 拥有冻结计划、接纳状态与交互；列表/详情都是投影。业务请求 ID 在一次意图中保持稳定；编辑只升级草稿和计划版本。重复提交返回同一任务；接纳后不能替换计划。提交或回答响应不明时保留原身份，查询或重试原命令，不能自动新建请求。服务实例变更后拒绝旧实例操作。

测试场景固定绑定 Test authority、模拟 Windows x86_64 设备与用户；不会借用实际宿主登录身份。软件申请停在管理员等待；诊断工具依次要求普通确认、隐私同意。维护窗口、重启提示、原计划参数复核和未知效果各有明确展示。普通用户不能提交管理员决定。取消交互不代表任务取消；超时、拒绝与取消都不完成测试流程。

服务没有 runner。FrozenPlan 仅绑定内嵌的不可执行测试字节及其摘要，目录资源版本摘要和 artifact 字节摘要分别计算，不代表实际软件产物验证。权限和适用性来自固定测试场景；真实授权、批准消费、执行接线与持久恢复仍由 C19/C20 负责。

页面卸载不发送取消，服务记录在进程存活期间保留；退出应用将丢弃所有内存测试数据，不承诺跨进程恢复。浏览器没有假服务实现，所有写按钮禁用。桌面 IPC 出错直接报错，不降级成浏览器样本。

浏览器快照的唯一生成入口：

```sh
node scripts/check-self-service.mjs --write
pnpm exec prettier --write apps/desktop/src/self-service/preview.ts
node scripts/check-self-service.mjs
pnpm exec vitest run apps/desktop/src
cargo test -p rss-mdm-desktop --locked
```

快照是 Rust 实际目录和请求的序列化投影，固定时间仅用于预览；TS 赋值类型检查与 CI 语义比较共同阻止漂移。IPC 测试使用真实 app manifest 和 Tauri MockRuntime 检查序列化、本地主窗口、非主窗口、远端 origin 和未知命令；这不代替真实 WebView 验收。

固定依赖：Tauri Rust 2.11.2 / tauri-build 2.6.2 / JS API 2.11.1，Vue 3.5.38，Node 24，pnpm 11.4.0。未升级或复制核心公共契约；无迁移、旧入口、兼容 adapter 或新远端 CI。

ref: Tauri [应用 ACL](https://github.com/tauri-apps/tauri/blob/tauri-v2.11.2/crates/tauri-build/src/acl.rs) 与 [IPC 测试](https://github.com/tauri-apps/tauri/blob/tauri-v2.11.2/crates/tauri/src/test/mod.rs)。本次新增代码为本仓实现，复用已提取的 UI 包，未从 prmonitor 新增源码提取。
