# 受控执行 MCP 适配器

将 MCP 工具映射到宿主绑定的执行服务，不持有批准、任务状态或幂等账本。工具与 schema 从 [Rust 声明](src/) 生成，协议版本和依赖由 manifest 持有。

可信主体由宿主 port 绑定，模型参数、metadata、工具注解和 provider 登录不能构造权限。所有读取和写入仍执行与传统 UI 相同的业务校验，跨主体查询不得泄露存在性。目录参数保留原始字节交给目录 owner，避免中间 JSON 转换丢失重复键和数字精度。

断线或响应丢失后使用原业务 ID 查询或重试；协议取消只结束等待，不取消已接纳任务。变更超时保留结果未知，不能声称未执行。服务独立持久保存业务幂等，RPC ID 不承担该职责。

transport 有界组帧、检查和编码；部分输出失败关闭连接，取消后的协议 ID 隔离，避免迟到响应污染后续调用。库只消费宿主持有的字节流，不启动子进程或监听网络。

宿主日志 subscriber 必须过滤 rmcp 内容日志，不直接接受任意 RUST_LOG 表达式。stdout 只写 MCP；目录与标识不得放入秘密。协议成功不证明 provider 原生工具旁路已封闭。

## 来源

- ref: rmcp transport/async_rw.rs@fd7811fdaa9fefa1c8034534b4d7a31c97204f89：[默认整行累积](https://github.com/modelcontextprotocol/rust-sdk/blob/fd7811fdaa9fefa1c8034534b4d7a31c97204f89/crates/rmcp/src/transport/async_rw.rs#L125-L160)，本 adapter 补齐 decode 前预算。
- ref: rmcp service.rs@fd7811fdaa9fefa1c8034534b4d7a31c97204f89：[请求分派与取消](https://github.com/modelcontextprotocol/rust-sdk/blob/fd7811fdaa9fefa1c8034534b4d7a31c97204f89/crates/rmcp/src/service.rs#L1564-L1659)，本 adapter 补齐入站许可与等待期限。
- ref: rmcp handler/server.rs@fd7811fdaa9fefa1c8034534b4d7a31c97204f89：[版本支持接口](https://github.com/modelcontextprotocol/rust-sdk/blob/fd7811fdaa9fefa1c8034534b4d7a31c97204f89/crates/rmcp/src/handler/server.rs#L378-L400)，显式限制实际支持集合。

- ref: rmcp-macros src/tool.rs@fd7811fdaa9fefa1c8034534b4d7a31c97204f89：[工具元数据](https://github.com/modelcontextprotocol/rust-sdk/blob/fd7811fdaa9fefa1c8034534b4d7a31c97204f89/crates/rmcp-macros/src/tool.rs)，本 adapter 用私有 typed descriptor 绑定元数据与分派，保留原始参数预算路径。
- ref: rmcp src/model.rs@fd7811fdaa9fefa1c8034534b4d7a31c97204f89：[结构化错误与结果](https://github.com/modelcontextprotocol/rust-sdk/blob/fd7811fdaa9fefa1c8034534b4d7a31c97204f89/crates/rmcp/src/model.rs)，目录诊断使用 adapter 自有的闭合 wire 投影。
