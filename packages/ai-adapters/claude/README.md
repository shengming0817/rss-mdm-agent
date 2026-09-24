# Claude Agent SDK adapter

使用 manifest 固定的官方 SDK 与随包 CLI，不调用系统 claude。可信 resolver 提供明确配置和凭据，支持 API Key 或 Auth Token，以及交给官方工具处理的已有配置；秘密不进入公共 wire 或诊断。接口见 [src](src/)。

原生 transcript 持有上下文，Host 稳定事件只用于展示。冷恢复重新准入且不复活旧回调；查无结果不能推断未执行。一次仅有一个原生 turn，产品持久队列由 Host 持有。取消请求、模型终态和真实进程退出分别记录，关闭未完成可用新预算重试。

原生执行工具关闭，普通会话仅允许结构化问题；受控组件提案走唯一 Host 工具接缝，仍需可信 verifier，不能产生执行批准。设置、plugins、hooks、MCP 与环境显式收窄，项目配置不能恢复旁路。SDK 限制不是 OS 沙箱证明，产品准入范围见[Host](../../../apps/ai-host/README.md)。

原生状态目录须属于当前用户且保持私有，拒绝别名或过宽权限；恢复仅使用 RSS 已登记的原生会话，不管理用户其他会话。

```sh
pnpm test:ai-claude
pnpm smoke:claude
```

配置端点 smoke 也可显式 `--credential-file PATH` 读取该文件的 env 中 URL/凭据/model；
不加载该文件的 hooks、权限或信任设置。结果写入被忽略的 `.local-ci-runs/claude-model.json`，
记录运行方式、结果与未覆盖项，不记录密钥、私有 URL 或对话原文。
证据类型为 real-sdk-configured-endpoint-smoke；兼容端点或代理的响应本身无法证明
上游实际模型身份，backendIdentityVerified 始终为 false，不据此宣称真实 Claude 后端验证。
失败证据保留闭合的阶段和 Result/Submission/Outcome 分类。关闭未确认时最多用新预算
重试一次；仍未停止则保留运行目录，并在 receipt 标记清理状态，避免删除仍在使用的目录。
完整本地 CI 不需要模型凭据，只运行确定性模型传输。


## 来源与许可

实现为本仓 TypeScript 重写，未复制 prmonitor 代码。对标
`prmonitor@4dcc87264ad740da6559824e0a8b04a1c2914d4b` 的 Claude CLI 生命周期/来源边界；
其 Rust CLI 调用、PR 业务、Tauri/SQLite、旧 bypass/自动批准方式均未迁入。

| 固定参考文件 | 本包落点与改写边界 |
|---|---|
| `src-tauri/src/review/engines/claude/engine.rs`、`manager.rs` | `src/adapter.ts`：仅对标生命周期/续接差异；改为 A01 port 和原生 SDK，无 review 任务语义 |
| `src-tauri/src/review/engines/claude/process.rs` | `src/runtime.ts`：进程结束证据概念；不复制 CLI 参数/stdio 解析或旧权限设置 |
| `src-tauri/src/review/session.rs` | Host 展示与原生 context 分离的参考；不迁移旧 session store，SDK 自有 transcript |

SDK `createSdkMcpServer` 在固定版本对 Zod record 的 bundled schema 转换会失败；桥参数
使用等价的开放 object schema，真实 SDK 首轮工具清单与调用测试覆盖该接缝。`alwaysLoad`
只保证唯一桥及时进入模型工具集，不增加自动许可。

一手固定参考：[@anthropic-ai/claude-agent-sdk@0.3.277 sdk.d.ts](https://unpkg.com/@anthropic-ai/claude-agent-sdk@0.3.277/sdk.d.ts)
的 Query、Options、SDKMessage、CanUseTool、PreToolUse、SpawnOptions；
[sdk.mjs](https://unpkg.com/@anthropic-ai/claude-agent-sdk@0.3.277/sdk.mjs) 的进程、MCP 与回调实现。
本包新代码 MIT；Anthropic SDK/CLI 保留其自身商业许可及使用条款，未重标为 MIT。
lock 固定 SDK 及平台包版本；pnpm release-age 例外仅列这些精确版本，未关闭全局供应链检查。
