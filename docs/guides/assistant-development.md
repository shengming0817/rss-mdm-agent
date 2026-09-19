# AI 助手页面

`apps/desktop/src/assistant` 消费 A04 公共客户端与 renderer，和自助服务共享 AppShell / NavigationList。首页仍是默认入口；切导航仅改变可见区域，会话控制器、草稿、原回调和附着订阅存活到应用卸载。真实服务由 `AssistantServices` 注入；默认入口没有注入时明确显示未连接。正式 Tauri/Host/provider/执行桥接由 C20 #2413 持有。

## 开发与验收

```sh
pnpm install --frozen-lockfile
pnpm dev:assistant-fixture
```

打开脚本打印的 loopback URL。开发入口先检查 Rust 生成绑定与 fixture 是否漂移。该入口在 `tests/assistant`，加载同一个产品 App；没有独立演示页或产品失败后的 fake fallback。它使用仅支持同进程恢复的 FakeHost 和普通会话/协议接缝，不连接真实模型、不执行 OS 操作。执行面板输入生成 fixture 的原始请求编号 `request-1`；fixture 的状态源是 Rust `execution-app`、实际 SQLite 和 S1 测试 runner。fixture API / HTTP transport 仅存在于测试目录，普通桌面 bundle 不包含它们。

```sh
pnpm build
pnpm check:assistant
pnpm test
```

浏览器验收使用 playwright-core 与实际 Chromium；macOS 默认系统 Chrome，其它平台设置 `AI_BROWSER_PATH`。覆盖 caller 列表与分页、同目录多会话、忙碌时排队/转向、跨窗口回答竞争、回调 abort、官方 A2UI 创建/更新/删除、HTML 惰性展示、可信设备详情及断线历史/恢复。协议包的独立 consumer 继续覆盖游标、背压、官方 ACP、旧 action 与 renderer 故障；页面测试不能替代它，fixture 不能作为真实引擎或 OS 安全证据。

## 状态与权威

- 会话只保存 ai-client 发布的投影。列表仅保留 namespace/status；命令携带原输入和 dispatch，临时 delta 不充当持久文本。稳定事件按 sequence/identity 排序，恢复和实时走同一个 reducer，不按时间或内容去重。
- queue/steer/cancel/resume 按当前 generation 的能力与运行坐标开放。运行中输入仍可编辑；未知接纳保留同一 commandId、截止时间和完整 payload，重试不创建新命令。回执只证明接纳，不产生乐观消息或模型终态。
- 普通问题保留原 command/run/generation；标准权限保留 AbortSignal 回调，失效后关闭。A2UI 仅通过 RuntimeSurface 的官方 catalog/action 接缝回答。已回答、过期、旧 generation、删除 surface 均不能继续提交。renderer 故障保留只读问题、选项和有界卡片内容；未知问题格式保留转义后的有界原始内容，不开放提交。
- 分离视图、AI 取消和设备执行事实独立。重新读取历史不恢复模型上下文；明确的 resume 不能新建 native session 冒充恢复。断线保留可见历史，不自动取消或重新派发。重新建立认证连接清空旧 caller 的展示和草稿。
- 执行面板只接收 `ExecutionApp::task_details` 的授权结果。一次 ReadResult + binding 复核获得同一 ExecutionRecord 的状态和冻结摘要；模型伪造 approved、管理员身份或设备目标只留在对话区域。有效期按本机时间标明尚未生效、有效或已过期，实际准入仍由执行服务核验；过期的批准阶段只作为历史记录。这里没有批准签发或执行提交接缝。摘要字段排除秘密、参数、进程输入和特权审计。Rust 是类型唯一声明源，`node scripts/check-execution-bindings.mjs --write` 更新绑定与五种真实 S1 fixtures。

## 直接版本切换

产品 wire 与协商为 V3 / contractVersion=3；标准 ACP1、A2UI v0.9.1 不变。删除旧 `status/accepted`，用携带完整不可变命令的 `command_accepted`；store accept 原子构造事件、命令与 receipt，禁止调用方提供另一份事件正文。AI SQLite schema 从1改为2；旧库只读拒绝，不原地迁移、清空或重建，也没有历史兼容 reader。

## 固定来源与改写范围

此页面与授权摘要为本仓新实现，未复制上游 UI 或 PR 业务。机制参考：

- ref: [Pi Desktop pi-event-router.ts @ ddc34405c2861e003e9446c663595458e76d23c7](https://github.com/FaqFirebase/pi-desktop/blob/ddc34405c2861e003e9446c663595458e76d23c7/src/main/pi-event-router.ts)：后台问题按 session 归属；本实现增加 generation/run 绑定与 AbortSignal 生命周期，不把内存 callback 当作重启保证。
- ref: [CloudCLI useSessionProtection.ts @ fd424f3fcd739371daeb6b173167e61f95270670](https://github.com/siteboon/claudecodeui/blob/fd424f3fcd739371daeb6b173167e61f95270670/src/shared/hooks/useSessionProtection.ts)：会话切换与过期更新；本实现复用持久投影，不复制其缓存为权威。
- ref: schemars1.2.2 `src/generate.rs` 与 json-schema-to-typescript16.0.0 `src/index.ts`：沿用本仓 Rust→JSON Schema→TypeScript 生成链，依赖及许可由 workspace lock 和包 owner 保留。

完整验收在已提交源码上执行本仓 `make ci CI_BASE=origin/develop`，结果绑定 SHA、lock 与运行环境；不将 #2413 的真实产品装配宣称为本项已完成。
