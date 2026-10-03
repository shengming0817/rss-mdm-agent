# 无 UI 受控执行应用服务

组合能力、授权、批准、生命周期与应用拥有的原子 JournalPort，桌面和 AI 共用此服务。SQLite 实现该端口；应用接收已打开的 journal，生产开库由 agent-service 组合根负责。应用由设备 owner 持有，窗口与模型只持请求/响应；切换用户不重绑定已冻结任务。

默认公共面供生产装配消费；确定性测试 runner 和 fixture 配置仅通过非默认 `test-support` 提供。测试与生产使用同一应用和规则核心，测试 runner 丢失内存历史后保留 Unknown，不根据计划合成成功。生产身份和真实 runner 由 agent-service 与平台 adapter 装配；即使测试支持被显式启用，生产启动仍在接触存储前拒绝 Test authority 和非 Real runner。

测试消费者在 dev 依赖中显式开启支持；`cargo test --workspace --locked` 保留现有测试。生成测试 fixture 时使用 `cargo run --locked -p execution-app --example task-details-fixtures --features test-support`。Cargo 会合并 feature，测试或 example 构建不证明生产依赖未启用测试支持；生产构建使用默认 feature，由现有边界检查核验实际 normal/build 依赖闭包。

提交重放保持原请求及冻结内容；显式新尝试才重新准入和消费批准。提交或回答响应丢失时保留原命令，内容改变不能复用键。核实不派发，取消不证明停止或回滚；停止请求失败仍需观察终止和效果。

Host 必须独立认证主体、来源、委托、能力、批准和时钟，事务内只消费已验证的本地快照。动作定位不附加结果读取权限；审计单独授权。任务详情从同一次授权读取生成，排除秘密和特权审计内容，前端绑定由 Rust schema 生成。

配置经可信审计后原子替换，失效时仅保留仍符合强制政策的配置，否则禁止新执行；读取、取消和核实保持可用。具体 API、错误与恢复动作见 [src](src/)。

## 来源


直接读取 kube-rs 固定 revision [`f3619c349faebb4af25df013498af5f2bb85d1f5` 的 `kube-runtime/src/controller/mod.rs`](https://github.com/kube-rs/kube/blob/f3619c349faebb4af25df013498af5f2bb85d1f5/kube-runtime/src/controller/mod.rs)，借鉴显式调谐建议与实际调度分离；本实现无上游源码复制或依赖，不引入 Kubernetes 资源模型、隐式自动重试或 worker。一次性权限与 SQLite 原子性直接消费仓内 C09/C18，无第二份机制。目标新源码沿用本仓 MIT。

本轮分权与诊断设计另读取 [Kubernetes v1.34.0 authorizer/interfaces.go](https://github.com/kubernetes/kubernetes/blob/v1.34.0/staging/src/k8s.io/apiserver/pkg/authorization/authorizer/interfaces.go) 的动作/资源属性、[apimachinery v0.34.0 types.go](https://github.com/kubernetes/apimachinery/blob/v0.34.0/pkg/apis/meta/v1/types.go) 的状态/原因分离，以及 [Axum 0.8.4 extract/state.rs](https://github.com/tokio-rs/axum/blob/axum-v0.8.4/axum/src/extract/state.rs) 的集中上下文与窄状态提取。仅借鉴模式，不复制代码、不新增依赖；Host 私有字段只经一个构造入口初始化。
