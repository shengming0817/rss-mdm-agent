# 桌面开发与验证

当前交付独立 Vue 组件包与 Tauri 2 桌面壳。应用持续显示“展示样本，无真实执行”；所有输入仅在窗口内存中，关闭后丢弃。无模型、后台服务、业务IPC、数据库或远程接线。

## 工程与启动

根 `Cargo.toml` 统一持有workspace、版本和Rust依赖，初始成员为 `apps/desktop/src-tauri`；后续能力按任务加入 `crates/<name>`，不预建空包。
根pnpm workspace管理 `apps/*` 与 `packages/*`。Vue页面、样本状态在 `apps/desktop/src`，原生窗口启动与打包配置在同应用的 `src-tauri`；通用组件位于 `packages/ui`。

使用 Node 24、pnpm 11.4.0 和 `rust-toolchain.toml` 固定的 Rust 1.96.0。先安装平台所需的[Tauri系统依赖](https://v2.tauri.app/start/prerequisites/)；macOS需要Apple命令行开发工具，Windows/Linux依赖以官方平台说明为准。

```sh
pnpm install --frozen-lockfile
pnpm dev                 # Tauri窗口；仅监听127.0.0.1:1420的开发服务
pnpm dev:web             # 浏览器中运行同一份样本
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

原生窗口另做人工或系统辅助功能验收：启动→看到样本标识→切换导航→输入发送→忙碌/取消→分栏→关闭退出。再停止开发服务器，启动发布程序重复验收，记录系统/架构和产物SHA。浏览器测试不代替此门。

C05只承诺实际记录的平台验证；Windows/Linux构建、安装签名、公证、升级、真实执行、模型接线与企业T3均不能由macOS样本推定通过。边界守卫扫描UI与桌面样本源码、Vue模板表达式及生产依赖，拒绝主机调用、网络能力与HTML注入入口。前端依赖和环境全局值采用allowlist，AST检查静态成员/别名、模板资源入口和原生表单提交；CSP逐directive/source检查，并独立突变CSP、capability、Cargo依赖和宿主注册。组件边界守卫是Medium检查，不是抵御任意恶意代码的操作系统沙箱。

宿主从唯一配置显式创建主窗口并挂接导航handler：macOS/Linux仅允许`tauri://localhost`，Windows仅允许当前配置对应的`http://tauri.localhost`，开发构建额外允许`http://127.0.0.1:1420`，拒绝凭据、非默认端口、其它origin及新窗口。CSP禁止外部资源、frame和表单提交，Tauri capability保持零命令权限；源码检查与实际运行限制分别承担职责。UI的`runtime.ts`聚合CSS，公开声明从无CSS导入的`src/index.ts`生成，归档消费者继续显式导入唯一`style.css`。

本地CI记录起止HEAD、CI_BASE对应merge-base及工作树状态；未提交源码或运行期间HEAD变动使验证失败。diff检查覆盖base到HEAD、暂存区和工作树。
