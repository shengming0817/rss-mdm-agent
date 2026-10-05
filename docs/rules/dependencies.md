# 依赖边界

具体crate边界由[PRD第6节](../product/rss-mdm-agent-prd.md#6-组件边界)持有。
纯核心与契约不依赖 UI、模型 SDK、数据库或 OS；AI 行为 ports 由 TS 契约包持有，Rust 仅消费生成 wire 与自身执行核心；adapter单向消费必要契约，组合根连接实际业务。必要基础类型和成熟库依赖允许，不为零依赖复制值类型。

- 本仓独立workspace、工具链、lock与发布周期；同workspace成员可用内部path，禁止以相邻RSS/rss-mdm/prmonitor目录path或patch作为交付前提。
- 客户端通过版本化 HTTP API 对接 rss-mdm；直接消费唯一纯协议包 rss-mdm-agent-wire，固定私有 Git URL 与完整 rev，由 Cargo.toml/Cargo.lock 持有版本与来源。禁止后端内部包、其它后端 Git/path/patch、浮动引用或备用来源。Cargo 下载私库源码时只构建协议包及公共基础依赖，不构建后端业务 workspace。
- Agent 远程协议权威由 rss-mdm-agent-wire 持有；消费者直接使用 DTO、静态约束、规范签名字节和摘要，不复制后端 schema、生成绑定、平行 DTO 或通用校验器。Agent 自己持有受信密钥、验签、设备/租户/注册世代绑定、可信时间、防重放、本机权限与资源预算；结构解码成功不授予执行权限。常规构建与生成不读取相邻后端 checkout。
- 依赖检查同时核对声明与 locked all-features metadata，覆盖普通、开发、构建、可选和平台依赖；协议例外精确绑定包名、版本、URL 和提交，拒绝依赖覆盖及来源旁路。私库凭据由系统 Git helper 或受保护构建环境提供，不提交到仓库。
- 客户端测试使用本仓协议 fixture 及实际本地存储/TLS 接缝；禁止构建另一产品、导入其私有夹具或读取其 checkout。部署服务的完整联调归独立产品验收，不进入常规 CI。
- AI/UI提取绑定来源commit、文件映射、权利/许可及改写证据，不链接整个prmonitor_lib，不带PR业务和旧不受控权限配置。
- 公共 API 通过现有单元、集成或产品调用路径验证；不维护平行的独立 consumer、pack/install 测试与回执框架。真实应用打包仍需验证运行资源完整性。
- 如需RSS公共机制，核验实际可用API与依赖闭包；缺口回原owner修复，不在产品仓维护通用机制副本。

工程入口和公共lock变更由单一集成人串行合并；各能力目录按任务owner隔离。

AI 产品 wire schema 唯一 owner 为 ai-contract；标准 ACP/A2UI 复用固定上游 schema。Rust 和 TS 生成物不得成为第二声明源。AI provider SDK 仅由对应 adapter 消费，协议 SDK 可作为契约验证的开发依赖；AI Host/Store 不复制 Rust 执行权威表。
