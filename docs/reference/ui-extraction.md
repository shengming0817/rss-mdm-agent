# C05 UI与桌面壳提取

任务：[#2398](https://dev.azure.com/shengming0923/rss/_workitems/edit/2398)。来源仓prmonitor固定commit `4dcc87264ad740da6559824e0a8b04a1c2914d4b`，不消费浮动分支或本地源码目录。

## 权利与许可证

固定来源树没有LICENSE/NOTICE文件，不能从可读取或提交作者推定开源许可。2026-09-09用户明确确认“自己的项目，保持mit授权，可以直接提取”，本次据该授权以MIT交付目标源码。根LICENSE和UI包内LICENSE一致；原有第三方权利不被本次授权覆盖。不修改来源仓，也不宣称来源历史提交包含MIT文件。

运行依赖Vue（3.5.38，MIT）外置于UI包，Tauri（2.11.2，MIT OR Apache-2.0）正常通过Cargo消费；未复制其实现。Tauri传递依赖各自许可随原包保留，Cargo.lock/pnpm-lock.yaml是实际依赖清单。开发工具按其发布包LICENSE消费，不复制为自有代码。

## 逐文件映射

源路径均相对固定prmonitor提交；目标路径相对本仓。表中源码以外的工程配置、测试、文档和原始SVG标识是本次新增。

| 固定源路径 | 目标路径 | 保留与改写 |
| --- | --- | --- |
| src/App.vue | packages/ui/src/components/AppShell.vue | 提取header/sidebar/content/footer布局；以slots替代所有装配、路由及store |
| src/pr/ProjectNav.vue | packages/ui/src/components/NavigationList.vue | 参考导航视觉结构，重新实现button、items/activeId/select；不复制PR层次和轮询逻辑 |
| src/SplitPane.vue | packages/ui/src/components/SplitPane.vue | 提取pointer/键盘/ARIA与布局；平衡默认比例、新增resize事件、动态边界与pointer取消处理 |
| src/splitRatio.ts | packages/ui/src/internal/splitRatio.ts | 提取归一化算法；默认0.5，增加非有限高度处理，删除业务说明 |
| src/review/ReviewStream.vue | packages/ui/src/components/MessageStream.vue | 提取纯文本/折叠/气泡展示，去除Review命名与Rust事件绑定 |
| src/review/types.ts（仅StreamItem） | packages/ui/src/types.ts | 改为UI-local MessageItem，itemId→id、message→assistant；导航和状态展示类型新建 |
| src/review/ReviewPanel.vue（输入片段） | packages/ui/src/components/MessageComposer.vue | 提取输入、折叠和IME行为；受控草稿与events替代store/API/PR/监听器，补legacy IME Enter处理 |
| src/StatusBar.vue（展示片段） | packages/ui/src/components/StatusList.vue | 按展示数据重写，无CLI探测、项目筛选、引擎启停或动作 |
| src/design/tokens.css | packages/ui/src/styles/tokens.css | 保留语义tokens及明暗色；改为组件类作用域，剔除来源业务注释 |
| src-tauri/src/main.rs、src-tauri/src/lib.rs（Builder入口） | apps/desktop/src-tauri/src/main.rs | 保留Windows GUI标记与Builder运行；新增仅创建受导航约束窗口的setup与启动失败诊断，无业务模块、command、plugin或状态注册 |
| src-tauri/build.rs | apps/desktop/src-tauri/build.rs | 只保留tauri_build入口；不复制远程Web占位/嵌入逻辑 |
| src-tauri/tauri.conf.json、capabilities/default.json | apps/desktop/src-tauri/tauri.conf.json、capabilities/main.json | 全新产品身份、窗口、最小权限和CSP；删除deep link、opener和业务资源 |
| vite.config.ts、src/main.ts | apps/desktop/vite.config.ts、src/main.ts | 固定本地开发端口和Vue挂载；不复制Tauri transport/环境探测与业务初始化 |
| 无 | apps/desktop/src/App.vue、style.css；packages/ui/src/styles/base.css、index.ts、packages/ui/runtime.ts | 唯一固定样本、基础作用域样式及公共导出，本次新建 |
| 无 | apps/desktop/src-tauri/icons/icon.svg、icon.png、icon.ico、icon.icns | 原创R字形标识；由Tauri CLI 2.11.2生成桌面图标，不沿用来源品牌图片 |

没有迁移PR历史、配置或数据库；不保留旧路径alias、Review类型或兼容shim。MIT包NOTICE随tarball分发。

## 固定对标

- ref: Spacedrive Cargo.toml@6dfeccf2113039e35f2ce735f945e70dc3e4ea45 — 已读取[固定源码](https://github.com/spacedriveapp/spacedrive/blob/6dfeccf2113039e35f2ce735f945e70dc3e4ea45/Cargo.toml)，参考根workspace包含嵌套Tauri成员；只借鉴组织，不复制该项目源码或许可。
- ref: Tauri crates/tauri/src/app.rs@tauri-v2.11.2 — 读取本地发布包Builder/run实现，使用最小宿主入口。
- ref: tauri-utils src/acl/capability.rs@2.9.2 — 读取本地发布包窗口capability边界；实际构建依赖以Cargo.lock为准。
- ref: Vue runtime-dom/dist/runtime-dom.esm-bundler.js@3.5.38 — 读取发布包createText/textContent实现，使用文本插值渲染。

接口与复现命令见[开发指南](../guides/desktop-development.md)。原生窗口与本地 CI 结果写入 PR，不将来源的跨平台能力当作本仓验证。

本轮修复对标：
- ref: [Tauri WebviewWindowBuilder navigation/new-window](https://github.com/tauri-apps/tauri/blob/tauri-v2.11.2/crates/tauri/src/webview/webview_window.rs) — 使用公开handler，保留产品自己的精确origin策略。
- ref: [TypeScript noUncheckedSideEffectImports](https://www.typescriptlang.org/tsconfig/noUncheckedSideEffectImports.html) 与 [Vite library CSS](https://vite.dev/guide/build#library-mode) — CSS聚合入口与声明图分离，公开声明应避免缺失引用。
- ref: [typescript-eslint static member utility](https://github.com/typescript-eslint/typescript-eslint/blob/main/packages/eslint-plugin/src/util/misc.ts) — 参考静态成员名解析；本仓直接复用已有TypeScript AST/checker，环境值走allowlist，不引入另一套解析依赖。
- ref: [Win32 MessageBoxW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-messageboxw) — 无owner窗口的同步错误框，用于Windows GUI启动失败。
- ref: [Node spawnSync](https://nodejs.org/docs/latest-v24.x/api/child_process.html#child_processspawnsynccommand-args-options) — 分别记录退出码、信号与spawn错误。
- ref: [WAI-ARIA log](https://www.w3.org/WAI/WCAG21/Techniques/aria/ARIA23) 与 [status](https://www.w3.org/TR/wai-aria/#status) — 消息追加与状态反馈的辅助技术语义。

## 助手页面机制参考


此页面与授权摘要为本仓新实现，未复制上游 UI 或 PR 业务。机制参考：

- ref: [Pi Desktop pi-event-router.ts @ ddc34405c2861e003e9446c663595458e76d23c7](https://github.com/FaqFirebase/pi-desktop/blob/ddc34405c2861e003e9446c663595458e76d23c7/src/main/pi-event-router.ts)：后台问题按 session 归属；本实现增加 generation/run 绑定与 AbortSignal 生命周期，不把内存 callback 当作重启保证。
- ref: [CloudCLI useSessionProtection.ts @ fd424f3fcd739371daeb6b173167e61f95270670](https://github.com/siteboon/claudecodeui/blob/fd424f3fcd739371daeb6b173167e61f95270670/src/shared/hooks/useSessionProtection.ts)：会话切换与过期更新；本实现复用持久投影，不复制其缓存为权威。
- ref: schemars1.2.2 `src/generate.rs` 与 json-schema-to-typescript16.0.0 `src/index.ts`：沿用本仓 Rust→JSON Schema→TypeScript 生成链，依赖及许可由 workspace lock 和包 owner 保留。

本轮修复的机制参考：

- ref: ACP TypeScript SDK 1.4.0 `dist/schema/types.gen.d.ts` / `PermissionOptionKind`：四种权限持续范围；采用已锁定官方包的定义。
- ref: [Node.js v24.0.0 lib/internal/abort_controller.js](https://github.com/nodejs/node/blob/v24.0.0/lib/internal/abort_controller.js)：owner cancellation 与超时信号；页面另外保留有界 Promise 结算及迟到资源清理。
- ref: [Vue v3.5.13 packages/reactivity/src/computed.ts](https://github.com/vuejs/core/blob/v3.5.13/packages/reactivity/src/computed.ts)：computed 依赖显式响应式输入，时间经共享 ref 更新。
