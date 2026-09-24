# 契约开发

执行契约由 [Rust owner](../../crates/execution-contract/src/) 声明；AI 产品 wire 由 [ai-contract schema](../../packages/ai-contract/schema/) 声明并生成 Rust/TypeScript。两者分别拥有执行权威和会话协议，不互相复制状态。

修改唯一声明后更新绑定，审阅生成 diff，再运行受影响的调用方测试：

```sh
pnpm generate:ai-contract
pnpm check:ai-contract
pnpm test:ai-contract
node scripts/check-execution-bindings.mjs --write
node scripts/check-self-service-bindings.mjs --write
```

执行契约的 schema/golden 从 crate examples 生成；只在有意改变契约时更新 golden，不能用重生成掩盖编码漂移。Rust 公共 API 说明由 rustdoc 持有，Cargo 测试与 Clippy 按受影响 crate 运行。共享绑定变更沿真实调用关系验证。

Schema 描述结构，不能替代动态预算、可信主体验证或 provider 恢复准入。新建和恢复均须重新核验当前 provider 配置与工具能力，原生历史不继承旧进程权限。不支持的格式明确拒绝，不增加兼容 reader、双写或迁移旁路。

来源和改写边界见[执行契约来源](../reference/contracts-extraction.md)与[AI Runtime 来源](../reference/ai-runtime.md)。
