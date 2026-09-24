# C01/C02 契约来源与改写

对应 [C01 #2394](https://dev.azure.com/shengming0923/rss/_workitems/edit/2394) 和 [C02 #2395](https://dev.azure.com/shengming0923/rss/_workitems/edit/2395)。本文记录 #2394/#2395 的历史纯契约交付；C02 当前代码已由 A01 生成绑定替换，历史文件清单不表示继续保留旧实现。固定产品来源见[来源索引](sources.md)。查阅日期：2026-09-12 UTC。

## 固定来源与唯一 owner

prmonitor 固定 revision：`4dcc87264ad740da6559824e0a8b04a1c2914d4b`；rss-mdm 产品基线：`589211a598d588508375f7e84742cfa0dc7d29ce`。源码提取探索逐文件读取固定 revision，不使用当前工作目录内容替代历史证据。

| 固定 prmonitor 文件 | 目标 | 参考与改写 |
| --- | --- | --- |
| src-tauri/src/review/engine.rs | crates/ai-session-contract/src/capability.rs、model.rs | 参考启动/续接差异，重新定义 engine 身份、能力和会话；删除 PR/Skill/Dedup 上下文 |
| src-tauri/src/events.rs、src/types.ts | crates/ai-session-contract/src/model.rs | 参考类型化流事件组织；重新定义 Conversation/Turn/Proposal，无 project、review、comment URL |
| src-tauri/src/review/session.rs | crates/ai-session-contract/src/capability.rs、tests/contract.rs | 参考中断中间态和 process generation；不复制 registry、Tauri、历史存储或执行编排 |
| src-tauri/src/review/engines/codex/protocol.rs | capability.rs、协议 fixtures 的需求依据 | 参考请求/中断差异；原自动批准与全权限配置只作为必须排除的反例，不复制 codec 或 RPC |
| src-tauri/src/review/engines/claude/process.rs | capability.rs、协议 fixtures 的需求依据 | 参考消息分段和特定 CLI resume 行为；不把来源行为泛化为本产品已验证能力 |
| src-tauri/src/review/engines/cursor/protocol.rs、process.rs | capability.rs、协议 fixtures 的需求依据 | 参考 capability/generation；不复制 force、sandbox-disabled、反向批准或进程代码 |

C01 为依照客户端 PRD 新建的本地执行契约，没有从服务端或来源仓复制执行域。Agent wire authority 继续由 rss-mdm 持有。来源仓不修改，新契约不保留来源类型 alias、旧字段或兼容 reader。

## 一手对标

- ref: serde-json-canonicalizer src/jcs.rs@db8ae9eaef2a17b8c829062aa8fcbea7635b33ae — 已读取[固定源码](https://github.com/evik42/serde-json-canonicalizer/blob/db8ae9eaef2a17b8c829062aa8fcbea7635b33ae/src/jcs.rs#L13-L53) 的 UTF-16 键排序及 L131–231 的数值格式化。正常依赖 `0.3.2`；调用前拒绝不安全整数，遵循 [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785.html)。
- ref: schemars schemars/src/generate.rs@ed6186319d5ebf1959a03f558df375d2bb5c44a6 — 已读取[固定源码](https://github.com/GREsau/schemars/blob/ed6186319d5ebf1959a03f558df375d2bb5c44a6/schemars/src/generate.rs#L108-L123)，显式选择 Draft 2020-12；正常依赖 `1.2.2`，不依赖未来 default。
- ref: ACP schema/v1/schema.json@f1293d8e43d09a6745ff8fe717f9acd8299591b7 — 已读取[能力声明](https://github.com/agentclientprotocol/agent-client-protocol/blob/f1293d8e43d09a6745ff8fe717f9acd8299591b7/schema/v1/schema.json#L2410-L2580)、[取消与尾部事件](https://github.com/agentclientprotocol/agent-client-protocol/blob/f1293d8e43d09a6745ff8fe717f9acd8299591b7/schema/v1/schema.json#L3565-L3586)；本地契约只提取必要语义，不承诺直接兼容 ACP wire。
- ref: Codex sdk/python/src/openai_codex/api.py@c4017a87aacc7558002b7cb510025e967c1d765e — 已读取[消息来源不建立授权](https://github.com/openai/codex/blob/c4017a87aacc7558002b7cb510025e967c1d765e/sdk/python/src/openai_codex/api.py#L625-L632)；C02 不将角色/来源或提案提升为执行权威。
- ref: MCP docs/specification/2025-06-18/server/tools.mdx@aa8ce049f089f92618340190d4ece141f663310d — 已读取[工具注解的信任边界](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/aa8ce049f089f92618340190d4ece141f663310d/docs/specification/2025-06-18/server/tools.mdx#L185-L195)，参数和注解不产生产品批准。

- ref: rust-url url/src/host.rs@d6ea13c5f8e7e6e627f6390161b3e185bda5e5ce — 已读取 [Host 解析与 IDNA/IP 规范化源码](https://github.com/servo/rust-url/blob/d6ea13c5f8e7e6e627f6390161b3e185bda5e5ce/url/src/host.rs)。正常依赖 `url =2.5.8`；本契约要求独立 scheme/host/port，在主机解析前拒绝 URL 组件/zone/通配符，解析后执行 DNS label 边界校验；不引入网络调用或自己实现 IDNA。
- ref: Rust library/std/src/process.rs@1.96.0 — 已读取 [Command::env 的平台语义](https://github.com/rust-lang/rust/blob/1.96.0/library/std/src/process.rs)，Windows 环境名不区分大小写。契约限 ASCII 名称并在 freeze 前拒绝冲突，再规范为大写；Unix 保留原大小写，runner 仅消费 frozen 值。
- ref: serde_json src/error.rs@de8500740cdcabffb9734f503e4889def823cf10 — 已读取 [Error::custom 的类型擦除](https://github.com/serde-rs/json/blob/de8500740cdcabffb9734f503e4889def823cf10/src/error.rs)。两契约各自拥有静态 kind/field/rule；仅跨 serde 恢复库内闭合诊断，不分析或公开用户/provider 错误正文，不增第三个 common crate。

## 权利与验证边界

目标 Rust 源码和 fixtures 为参考后重新设计，按本仓 MIT 交付。prmonitor 固定树缺少 LICENSE/NOTICE；已有用户自有项目 MIT 授权记录见 [C05 来源记录](ui-extraction.md)，但本次不将 C05 文件清单当作 C02 复制清单。

未复制 prmonitor 或 ACP/Codex/MCP 源码、生成 schema、示例数据和版权正文；无需为不存在的源码复制添加第三方 NOTICE。serde、serde_json、schemars、thiserror、sha2、url、canonicalizer 和测试用 jsonschema 通过 Cargo 发布包正常消费，版本/校验和由 Cargo.lock 持有，许可证随原包保留。若后续复制源码，应单独核对对应固定版本权利并记录范围。

来源不代表当前产品验证结果，测试操作由对应 owner 持有。

A01 当前真源、生成工具、源码与许可见 [AI Runtime 来源](ai-runtime.md)。历史 C02 的取消/未知状态、提案不授予权威、输入预算及安全诊断语义在 V2 重新验证；没有保留 V1 运行入口或数据迁移器。
