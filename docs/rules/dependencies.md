# 依赖边界

具体crate边界由[PRD第6节](../product/rss-mdm-agent-prd.md#6-组件边界)持有。
纯核心与契约不依赖 UI、模型 SDK、数据库或 OS；AI 行为 ports 由 TS 契约包持有，Rust 仅消费生成 wire 与自身执行核心；adapter单向消费必要契约，组合根连接实际业务。必要基础类型和成熟库依赖允许，不为零依赖复制值类型。

- 本仓独立workspace、工具链、lock与发布周期；同workspace成员可用内部path，禁止以相邻RSS/rss-mdm/prmonitor目录path或patch作为交付前提。
- 跨仓消费固定版本/hash artifact或明确批准的完整Git revision与lock，不浮动跟踪分支，不把Git消费称为registry发布。
- Agent wire由rss-mdm先发布，本仓再消费；本地execution-contract不建立第二套远程协议或设备身份权威。
- AI/UI提取绑定来源commit、文件映射、权利/许可及改写证据，不链接整个prmonitor_lib，不带PR业务和旧不受控权限配置。
- 公共 API 通过现有单元、集成或产品调用路径验证；不维护平行的独立 consumer、pack/install 测试与回执框架。真实应用打包仍需验证运行资源完整性。
- 如需RSS公共机制，核验实际可用API与依赖闭包；缺口回原owner修复，不在产品仓维护通用机制副本。

工程入口和公共lock变更由单一集成人串行合并；各能力目录按任务owner隔离。

AI 产品 wire schema 唯一 owner 为 ai-contract；标准 ACP/A2UI 复用固定上游 schema。Rust 和 TS 生成物不得成为第二声明源。AI provider SDK 仅由对应 adapter 消费，协议 SDK 可作为契约验证的开发依赖；AI Host/Store 不复制 Rust 执行权威表。
