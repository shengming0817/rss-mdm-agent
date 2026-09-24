# ai-session-contract

AI 产品 wire 的 Rust 绑定与安全解码。唯一声明在 [AI schema](../../packages/ai-contract/schema/runtime.schema.json)，生成物随源码交付，不能手写修改；生成操作见[契约指南](../../docs/guides/contracts-development.md)。

外部字节经过有界解码，普通 serde DTO 不替代完整契约校验。生成类型和错误保持脱敏。Rust 不实现 AI Host/Store/provider ports，也不从模型输出、能力声明或工具响应签发执行权限。

取消派发、原生上下文恢复和业务执行恢复分别归各自 owner。不支持的格式明确拒绝，不补造旧命令身份或输入。
