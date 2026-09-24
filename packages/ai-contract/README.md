# AI Runtime 公共契约

产品 wire 唯一声明在 [runtime.schema.json](schema/runtime.schema.json)，TypeScript ports 和纯转换在 [src](src/)，Rust 消费生成绑定。生成流程见[契约指南](../../docs/guides/contracts-development.md)。标准 ACP/A2UI 保持上游 owner。

外部字节必须有界解码，DTO 或类型断言不构成校验。身份来自可信组合根，模型、卡片和配置声明不能签发权限。新建与恢复均重新验证 provider binding、能力和受控工具证据；历史不继承旧 incarnation 的权限。

命令接纳、派发、运行、模型终态、进程退出与设备效果是不同事实。未知派发先核实原尝试，只有可信未提交证据才允许在原期限内重试；重试保持完整命令身份。持久事实提交后才发布，临时 delta 不替代稳定历史。

展示恢复不恢复原生上下文或问题回调。回答与问题状态原子提交，过期、旧 generation、已删除卡片不得继续操作；普通回答不能成为执行批准。标准 ACP prompt 保留最终响应语义，接纳回执由协商扩展表达。

测试替身与公共 conformance 提供确定性行为验证；真实 SQLite、provider 和 OS 效果由各 owner 验证。API、预算、字段与版本由源码和生成物持有，不提供旧格式兼容、双写或隐式导入。来源和许可见[AI Runtime 来源](../../docs/reference/ai-runtime.md)。
