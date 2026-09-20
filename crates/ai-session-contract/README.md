# ai-session-contract

[AI Runtime V5](../../packages/ai-contract/README.md) 的 Rust wire consumer。唯一 schema owner 为 `packages/ai-contract/schema/runtime.schema.json`；本 crate 的 generated.rs 和 schema.json 均由固定生成链投影，禁止手写修改。生成 Rust 随源码交付，独立消费不需要 Node、相邻 package、build.rs 或联网生成。

公共 `decode`/`encode` 使用必填 Limits，拒绝重复键、未知版本/字段、非法 UTF-8、预算越界、安全整数越界及关联冲突；fingerprint 使用 JCS/SHA-256 并先执行同一校验。生成类型的 Debug 由生成器统一脱敏，ContractError 只返回闭合 Diagnostic。直接 serde 反序列化 DTO 不代替完整契约校验。

Rust 不实现 AI Host/Store/provider 行为 port，不签发可信主体或批准。模型输出、tool proposal/response、capability 声明都不是执行 Evidence。取消派发不是终态；native context 恢复属于 provider，业务执行恢复继续归 Rust 执行服务。

V5 完整替换 V1–V4，无兼容 reader/alias/历史导入；历史 #2395 由 Git 和 PR 保留。消费者更新公共 API 并重新建立会话，不能给旧命令补造派发身份或用户输入。

```sh
cargo test -p ai-session-contract --locked
cargo run -p ai-session-contract --example ai-session-consumer --locked -- packages/ai-contract/src/testing/fixtures.json
pnpm generate:ai-contract
pnpm check:ai-contract
```

共享 golden 的字节拒绝、往返和摘要证明为 T1，不是实际模型、SQLite、OS 或安全隔离验证。
