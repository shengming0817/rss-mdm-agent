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

执行契约的 schema/golden 从 crate examples 生成；只在有意改变契约时更新 golden，不能用重生成掩盖编码漂移。Rust 公共 API 说明由 rustdoc 持有，Cargo 测试与 Clippy 按受影响 crate 运行。共享绑定变更沿真实调用关系验证。执行请求、工具定义和详情类型统一生成到 `execution-bindings`，Host 与桌面共用经校验的业务 ID 提取入口。AI 契约只检查当前 schema、类型、验证器和运行资源一致性。普通展示、属性和诊断扩展保留未知 key，并受字节、深度和节点预算约束；核心身份、授权、状态、顺序和恢复仍使用明确契约。扩展字段不触发全局升版。

Schema 描述结构，不能替代动态预算、可信主体验证或 provider 恢复准入。新建和恢复均须重新核验当前 provider 配置与工具能力，原生历史不继承旧进程权限。不支持的格式明确拒绝，不增加兼容 reader、双写或迁移旁路。

来源和改写边界见[执行契约来源](../reference/contracts-extraction.md)与[AI Runtime 来源](../reference/ai-runtime.md)。

Agent HTTP API 直接消费固定 Git rev 的 `rss-mdm-agent-wire`。DTO、静态约束、签名字节和摘要由协议包唯一维护；本仓私有 HTTP 调用声明关联这些类型、method/path、成功状态和媒体类型，不生成或复制后端 DTO/schema，也不执行运行时 JSON Schema 校验。升级时修改 workspace 依赖的完整 rev、更新 Cargo.lock，核验固定编码向量及受影响通信/执行接缝。依赖检查从 Cargo 实际解析的同一 Git 来源读取指定提交的 HTTP 制品，复用该提交的纯提取/来源校验工具，不构建后端。协议测试用此制品核对调用声明和实际请求/响应，包括 200/null、204 空响应与下载语义；有意畸形响应属于显式故障注入。来源无法取得时失败，不切换到相邻 checkout、浮动分支或备用来源。

私库读取使用系统 Git credential helper 或受保护构建凭据；可通过 `CARGO_NET_GIT_FETCH_WITH_CLI=true` 使用 Git helper，不在 manifest 或日志中写凭据。`node scripts/check-agent-dependencies.mjs` 核对完整依赖闭包，常规 `make ci` 已包含该检查。Agent 继续拥有验签、身份绑定、防重放、本机权限与预算；网络解码成功不授予本机执行权限。

## Windows 系统采集与当前候选

Collection 消费固定正式 wire，三个本地库直接创建当前 V1；安装预检和运行期校验当前结构与身份，
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

本仓当前签名向量来自固定协议候选，仅证明 wire。正式标准包冻结任务样本必须来自生产者的
实际蓝图/任务路径，不能以手写同形 fixture 替代。宿主恢复测试也不证明 Windows 原生 API 支持；
Windows 编译、真机及正式包 Collection 结果由 #2728 独立平台验收记录。

## macOS 系统采集

macOS 沿用上述正式 wire、inline 结果、journal 与用户 helper，新增 security/accounts/certificates/updates
四类原生 engine。工具由同一固定路径/参数定义持有，broker 按当前 query 校验；没有脚本兜底或通用命令入口。

- Gatekeeper/XProtect/系统扩展分别报告配置、存在和可观察运行状态。XProtect 的版本、扩展
  `activated enabled` 不证明实时防护在运行。FileVault、SIP、防火墙及本地 token 独立解释；
  Bootstrap Token 输出只表示本机 `profiles` 的声明，不证明服务端实际托管。
- 账户仅查询本地 OpenDirectory，按批准 Local/Selected GeneratedUID 过滤；管理员仅直接成员，
  不递归组、不读凭据属性来推断 enabled。会话只覆盖当前 console，因此结果为 Partial。
- 证书仅指定 System/SystemRoots/Login 库的公开元数据；Login 绑定当前用户 GeneratedUID，
  不解锁、不提示、不读私钥、不联网验证信任。证书原始编码仅在进程内解析和计费。
- 指定软件只观察匹配 bundle/team 的注册应用与签名进程，不启动目标软件。Agent 指当前固定
  agent-service 可执行身份，不表示桌面应用；health 保持 unknown。公开 TCC preflight 只查调用方，
  不能证明目标软件权限，因此请求的权限均明确 unknown，不读 TCC 数据库或个人文件探测权限。
- 更新只读已有 SoftwareUpdate 缓存、InstallHistory 与当前 OS/build，Online 明确 Unsupported。
  缓存不证明最新适用集合，历史不证明当前安装全集；当前 OS 是已安装状态的部分证据。
  当前已核实的缓存只有 LastSuccessfulDate，没有扫描开始时间；固定协议不允许仅 finishedAt，
  因此不制造区间，scan 时间为空且结果 Partial。缺缓存/权限/格式错误不得变成 Complete 空清单。
- 当前批准的来源没有独立、可靠的硬件加密状态或全局待重启读取接口，这两项返回 unknown +
  Unsupported；不以 CPU/FileVault 或更新条目的 restart 要求替代。未知系统格式不走历史兼容解析。

宿主 provider 验证可显式运行
`cargo test -p agent-service host_system_queries_through_confined_broker -- --ignored --nocapture`。
该入口读取机器安全状态、系统根证书、当前调用账户/一个不存在的指定账户、指定应用基础状态和本地更新缓存，日志只记录状态与计数，
不访问个人 Login keychain。纯解析样本标记真实无身份输出与合成边界；开发机通过不代表其他架构、
企业身份/TCC/用户钥匙串或完整部署链验收，后者由独立 T3 持有。
