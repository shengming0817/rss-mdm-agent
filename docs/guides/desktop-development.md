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

Rust build/test前需先 `pnpm build`，生产资源由Tauri嵌入。Cargo workspace根的 `target/release/rss-mdm-desktop`（Windows为 `.exe`）是发布可执行程序，可在无Vite服务时启动。启动失败保留错误，不能以编译成功代替窗口验收。

## 组件消费

应用通过 `@rss-mdm-agent/ui` 公开入口导入组件/类型，并显式导入 `@rss-mdm-agent/ui/style.css`。Vue是peer dependency；CSS仅在 `.rss-ui` 下生效，不修改宿主body。导出 `AppShell`、`NavigationList`、`SplitPane`、`MessageStream`、`MessageComposer`、`StatusList`；类型见包内 `src/types.ts`。

- AppShell持有header/navigation/default/status slots；宿主给它明确高度。
- NavigationList只发出启用项id，由宿主更新activeId；原生button支持键盘操作。
- SplitPane为上下分栏，初始比例0.5、最小比例0.1；初始比例只在挂载时使用，动态minRatio会重新约束当前比例并更新ARIA。resize输出比例，宿主自行决定是否保存。
- MessageStream只显示文本，reasoning默认折叠，不自动打开链接或解释终端控制序列。
- MessageComposer的modelValue由宿主持有；submit返回去除两端空白的文本，不自行清空。busy/disabled/折叠/空白阻止提交；canCancel独立表示宿主允许取消，disabled阻止取消。取消事件不证明任何任务已经终止。
- StatusList消费明确label/message/tone，不探测系统、认证或引擎。

`pnpm check:consumer` 将UI包打成归档，在仓外临时workspace离线安装并消费所有公开组件、类型和CSS，检查生产构建及安全文本渲染；退出清理临时目录。首次先完成正常安装，以填充依赖缓存。

## 验收范围

`make ci` 包含frozen install、前端构建、类型、组件与样本测试、边界守卫及负例、独立归档消费、文档/diff、Cargo build/test/fmt/clippy；报告写入被忽略的 `.local-ci-runs/latest.json`，包含源码SHA、时间、平台及逐步结果。无相关Rust行为时不添加空测试。

原生窗口另做人工或系统辅助功能验收：启动→看到样本标识→切换导航→输入发送→忙碌/取消→分栏→关闭退出。再停止开发服务器，启动发布程序重复验收，记录系统/架构和产物SHA。浏览器测试不代替此门。

C05只承诺实际记录的平台验证；Windows/Linux构建、安装签名、公证、升级、真实执行、模型接线与企业T3均不能由macOS样本推定通过。组件边界守卫是Medium检查，不是抵御任意恶意代码的操作系统沙箱。
