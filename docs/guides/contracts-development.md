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

执行契约的 schema/golden 从 crate examples 生成；只在有意改变契约时更新 golden，不能用重生成掩盖编码漂移。Rust 公共 API 说明由 rustdoc 持有，Cargo 测试与 Clippy 按受影响 crate 运行。共享绑定变更沿真实调用关系验证。执行请求、工具定义和详情类型统一生成到 `execution-bindings`，Host 与桌面共用经校验的业务 ID 提取入口。AI 契约生成检查会对比 `CI_BASE`（默认 `origin/develop`）的共同基线：结构变化必须提升统一协议版本，记录版本、协商与运行包保持一致；描述性文案调整除外。

Schema 描述结构，不能替代动态预算、可信主体验证或 provider 恢复准入。新建和恢复均须重新核验当前 provider 配置与工具能力，原生历史不继承旧进程权限。不支持的格式明确拒绝，不增加兼容 reader、双写或迁移旁路。

来源和改写边界见[执行契约来源](../reference/contracts-extraction.md)与[AI Runtime 来源](../reference/ai-runtime.md)。

Agent HTTP API 直接消费固定 Git rev 的 `rss-mdm-agent-wire`。DTO、静态约束、签名字节和摘要由协议包唯一维护；本仓不生成后端绑定，也不执行通用后端 JSON Schema 校验。升级时修改 workspace 依赖的完整 rev、更新 Cargo.lock，核验固定编码向量及受影响通信/执行接缝。来源无法取得时失败，不切换到相邻 checkout、浮动分支或备用来源。

私库读取使用系统 Git credential helper 或受保护构建凭据；可通过 `CARGO_NET_GIT_FETCH_WITH_CLI=true` 使用 Git helper，不在 manifest 或日志中写凭据。`node scripts/check-agent-dependencies.mjs` 核对完整依赖闭包，常规 `make ci` 已包含该检查。Agent 继续拥有验签、身份绑定、防重放、本机权限与预算；网络解码成功不授予本机执行权限。

## Windows 系统采集与当前候选

Collection 使用 V3，通信库存储格式为 v7；安装预检和运行期拒绝旧通信库，
不提供原地转换、补字段或旧 DTO 读取。旧实例由原版本完成未决任务及回传，保留注册和 journal；
不能通过清库、改版本号或重新注册绕过。macOS 已有用户采集须绑定本地目录的 GeneratedUID，
不能仅凭可复用的数字 UID 恢复用户权限。

Windows 安全、账户、证书及更新使用固定原生 query worker。包含 System recipe 的结果只走
有界 inline CollectionResult，不转成原始 chunk。批准 store 每次只读取机器或当前已认证用户，
证书仅返回元数据；管理员成员保留直接 SID，不展开域目录。SAM 与固定系统主体读取不能证明
所有服务/虚拟主体，Local 账户结果保留 Partial；Selected 只接收批准集合。

WUA Local 不联网，Online 仅用于明确批准的 Scan/Available，始终禁止自动 WUA 升级及更新部署。
本地缓存或历史无法提供可信原扫描区间时为 Partial/NotScanned，不用本次读取时间代替；
待重启状态独立观察。查询进程关闭不证明系统更新服务内部查询已经终止。

本仓 V3 签名向量来自固定协议候选，仅证明 wire。正式标准包冻结任务样本必须来自生产者的
实际蓝图/任务路径，不能以手写同形 fixture 替代。宿主恢复测试也不证明 Windows 原生 API 支持；
Windows 编译、真机及正式包 Collection 结果由 #2728 独立平台验收记录。
