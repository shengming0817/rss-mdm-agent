# C04/C06/C07 执行核心来源

对应#2397、#2399、#2400；查阅日期2026-09-13 UTC。能力为按本产品PRD重新设计的Rust实现，不复制prmonitor执行/批准方式，不修改来源仓。原产品来源revision继续以[来源索引](sources.md)为准。

## 一手Rust源码对标

- ref: tokio tokio/src/sync/oneshot.rs@75fef53d0a8590c2d1dbb63672aa7b7d1ef51155 — [一次发送与关闭竞争](https://github.com/tokio-rs/tokio/blob/75fef53d0a8590c2d1dbb63672aa7b7d1ef51155/tokio/src/sync/oneshot.rs#L1516-L1548)。采纳单一赢家；交互须可恢复并区分回答/取消/过期，故使用显式状态和条件提交输出，不依赖channel或drop。
- ref: cargo crates/cargo-platform/src/lib.rs@7941be6fb416b4cd9666aef7b858dfea25587a8c — [要求与外部事实分离](https://github.com/rust-lang/cargo/blob/7941be6fb416b4cd9666aef7b858dfea25587a8c/crates/cargo-platform/src/lib.rs#L24-L42)。采纳纯匹配；产品必须保留四种状态，不能压缩为bool。
- ref: wasmtime crates/wasi/src/ctx.rs@817c58787f432bcdbbb87679011f72c5bc80dbda — [默认能力边界](https://github.com/bytecodealliance/wasmtime/blob/817c58787f432bcdbbb87679011f72c5bc80dbda/crates/wasi/src/ctx.rs#L47-L63)、[显式路径权限](https://github.com/bytecodealliance/wasmtime/blob/817c58787f432bcdbbb87679011f72c5bc80dbda/crates/wasi/src/ctx.rs#L253-L272)。借鉴逐维显式约束和默认拒绝；C06只匹配快照，不创建WASI或原生沙箱。
- ref: cedar cedar-policy/src/api.rs@2f4019fd645cc8d4a4c0c1f8bd0280c77d754e28 — [principal/action/resource/context](https://github.com/cedar-policy/cedar/blob/2f4019fd645cc8d4a4c0c1f8bd0280c77d754e28/cedar-policy/src/api.rs#L5091-L5159)；ref: cedar cedar-policy-core/src/authorizer.rs@2f4019fd645cc8d4a4c0c1f8bd0280c77d754e28 — [显式拒绝优先](https://github.com/cedar-policy/cedar/blob/2f4019fd645cc8d4a4c0c1f8bd0280c77d754e28/cedar-policy-core/src/authorizer.rs#L368-L386)。采纳请求/可信规则/裁决分离；本产品另有ApprovalRequired，首版使用Rust精确规则，不引入Cedar或通用IAM。

以上源码已在探索阶段按固定revision读取，仅借鉴语义，不复制源码/数据/版权正文；目标实现按本仓MIT交付。实际基础依赖由Cargo.lock持有版本与校验和。C01规范编码仍使用既有owner，未增加第二套摘要算法。
