# service-catalog

目录拥有不可变内容、精确选择、参数投影与展示解释；能力、授权、资源解析和执行由调用方的对应 owner 持有。冻结内容不能认证发布者，listed 不能证明可申请或可执行。离线缓存可浏览，接纳新变更仍需当前可信核验。

人用表单与 AI 输入共用参数声明和运行时校验；JSON Schema 不替代聚合预算、重复键检查和可信身份验证。秘密仅以引用传递。实现与 API 说明见 [src](src/)，schema 由 [examples](examples/) 生成；不要手写第二份格式或摘要定义。


rss-mdm-agent 拥有目录和本地执行契约；rss-mdm 拥有 Resource、Group/Scope/Policy、软件源发布、产品授权与 Agent wire。C03 单向依赖 execution-contract 的 canonical 值类型，不依赖相邻后端仓，不引入第二个 shared-types 包或 facade。

| 目录语义 | 后续 rss-mdm adapter 责任 |
| --- | --- |
| authority enterprise{id,tenant} | 按可信接线选择 issuer/tenant；不能由目录或模型自行声明认证成功 |
| resource.reference.id/revision | 精确映射 Resource Version.resource/label；标识语法不同则显式拒绝，禁止截断、补前缀或模糊查找 |
| resource.versionDigest | 核对 Resource Version.digest，保留其“整个资源版本定义”的语义；不是下载文件 SHA-256 |
| selector.platform/architecture/key | 显式映射目标平台、架构和资源变体；x86_64/aarch64 与后端 X86_64/Aarch64 一一对应，禁止架构/变体回退 |
| action | 由具体执行 owner 验证动作与选中资源声明相容；目录字符串不是执行命令 |
| SelectedOperation | 后续 host 必须消费完整 pin、selector、参数和要求，解析实际 artifact 后核对字节长度/SHA-256并冻结计划 |

后端 Resource Rust 类型不是共享 wire；目录不复制其 canonical 编码、生命周期和业务枚举。tool 是目录展示分类，不强制等同于后端 Resource kind。实际 wire 由后端 producer 发布后按固定版本/hash消费，本次不预建 adapter/resolver。

目录内没有 actor/device/target/批准字段。执行请求上下文由 host 显式绑定，目标不能从 AI 参数或同名 OS/产品账号推断；外部展示说明中的 Target 只是关联坐标，不构成认证。

## 不兼容与可扩展性

新增软件、脚本、动作和枚举选项通过目录数据扩展，生成新目录 revision/digest。新增参数类型/字段语义时更新唯一类型、schema、runtime 和实际消费者；不兼容时提高目录格式版本，旧客户端报告 unsupported，而不是忽略新执行约束。没有旧格式兼容、默认变体或 latest 回退。

目录新版本不会替换已冻结计划。是否继续执行旧计划由执行 owner 核验原 pin、当前政策和批准；目录刷新或缓存本身均不能证明批准有效/失效。


来源与后端观察 revision 见[来源索引](../../docs/reference/sources.md#服务目录对齐)。
