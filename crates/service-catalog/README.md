# service-catalog

[C03 #2396](https://dev.azure.com/shengming0923/rss/_workitems/edit/2396) 的独立目录核心。目录只拥有不可变内容、精确选择、参数规则与展示解释；没有能力匹配、授权、后端解析、下载或执行接口。

## 公共入口与信任边界

- `decode_catalog(bytes, CatalogLimits)` / `FrozenCatalog::freeze(dto, limits)`：严格有界解析、语义检查、规范排序和摘要绑定。冻结只证明内容一致，不能认证发布者。
- `snapshot()` / `reference()`：只读内容和 `authority + identity{id,revision} + digest`。一个快照只有一个命名空间；企业 tenant 不可省略，local/test 不伪造企业主体。
- `projection(item, variant, ParameterLimits)`：同时提供表单字段与 AI Draft 2020-12 `input_schema()`，不维护两份约束。`validate(bytes)` 是共同参数入口，输出 `InputValue`。
- `select(bytes, CatalogLimits, ParameterLimits)`：接受 `catalog / itemId / variantId / arguments`，返回私有字段的 `SelectedOperation`，保留完整资源绑定、参数声明、要求及规范参数。返回的 SelectionRef 还绑定规范化参数摘要；省略默认值与显式相同默认值等价，修改有效参数使原外部说明不再匹配。没有 `ExecutionRequest` 快捷转换、可信主体或执行 permit。
- `availability(now_unix_ms)`：解释记录状态；withdrawn 优先，其次 `now >= expiresAt` 为 expired，其余为 listed。listed 仅指这份快照的记录，不表示最新目录、可申请或可执行。到期/下架仍可检查内容。
- `external_status(target, now, assessment)`：只核对外部展示说明与精确选择、device/platform/user 目标及 UTC 时间窗口的关联。缺失、过期或早于检查时间均为 unknown；关联错误拒绝。只有负向说明，不重做 C06 能力算法、不验证签发者、不生成授权。

C07/execution-app 及后续企业接线负责可信主体、当前资源发布态、能力、政策、批准与执行前复核。模型、OS 登录或合法 DTO 不能授予权限。离线缓存可浏览，但无当前可信核验就不能据此接纳新变更。

## 字段与版本

快照包含 schemaVersion、authority、identity、exclusive expiresAtUnixMs 与 items。项目包含名称/说明/分类、software/script/tool 展示类型、aiDiscoverable、required/optional/request 分发提示、listed/withdrawn 状态与非空操作集合。提示不签授权。

操作独立绑定 action、资源 ID/revision/versionDigest、platform/architecture/key、参数与 capability/runAs/interaction/evidence 要求。目录 operation ID、action 与资源 selector key 是不同坐标，不可互相代替。platform 复用 execution-contract；architecture 使用 x86_64/aarch64。Linux 类型不承诺后端 Linux 受管能力。

V1 SHA-256 输入为域 `rss-mdm-agent/service-catalog/v1\0` 后接 JCS 内容。items/operations 按 ID 排序，capability/evidence 集合排序，JSON 对象使用规范键顺序；显示选择项数组顺序保留。全部内容包括上下架、展示、参数默认值和期限均参与摘要。item ID、operation ID、参数名及要求集合不得重复，空目录允许，空操作集合和空 evidence 要求拒绝。名称、分类和参数 title 不得仅空白。

`schemaVersion`、目录 revision、资源 revision、内容摘要各自独立。资源 revision 作为 opaque 精确标签传递，从不解析 `latest` 或其它别名；是否真正不可变由资源 owner 核验，字符串合法本身不是证据。不同内容即使被错误标为同 revision，也有不同摘要并拒绝旧选择。

首版只接受 schemaVersion 的整数 1；未知字段、类型、版本明确拒绝。扩展新项目/动作/资源/参数枚举是数据变化；新增参数类型或格式语义时更新唯一实现、schema、消费者与测试，不兼容格式升级版本。本次无历史格式迁移，不提供旧格式 alias、shim、双读或回退。未来实际持久消费者的切换须由其 owner 明确处理。

## 参数规则与预算

字段 key 使用 execution-contract Id；字段 title/description 为纯文本。规则有 string（字符长度、choices、default）、integer（安全范围、choices、default）、boolean（default）、secretReference（仅 id/revision）。choices 是非空无重复集合，默认值必须合法且只能用于 optional 字段。required 缺值失败，显式 null 不视作缺值。

整数限定 ±(2^53−1)，允许数学等价的 3/3.0/30e-1 并规范为 3，不接受字符串数值。解析前检查原数字 token，拒绝因浮点舍入/下溢才成为整数的数值。字符串长度按 Unicode scalar 计数，UTF-8 字节另受宿主预算限制。secretReference 不允许 default/choices/秘密正文；只映射到 `InputValue::Secret`，不解析秘密。未知参数、重复键和不支持的嵌套业务结构拒绝。错误使用闭合 CatalogError 类别与 Limit/DefinitionRule/ArgumentRule 静态坐标，区分各预算、定义和参数规则，不回显字段名或值。

`CatalogLimits` 的 raw/canonical bytes、depth（1..=64，根为1）、nodes（含容器）、string bytes（含 key）、collection items 均显式正值；解析过程中消耗节点/深度/集合预算，先拒绝超总字节输入。typed freeze 先用有界 writer，再经过同一解析路径。

`ParameterLimits` 的 bytes、string bytes、parameters 均显式正值。参数 bytes 限制紧凑参数 JSON，在默认值展开前后均核验；独立参数字节入口也限制原始输入。声明数量受 parameters 限制。JSON Schema 只描述结构，不表达聚合字节预算、重复键和信任检查；所有入口必须调用共同 runtime。禁止 UI/AI 自行弱化 schema 或自行补默认值。

## 验证

```sh
cargo test -p service-catalog --locked
cargo clippy -p service-catalog --all-targets --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc -p service-catalog --no-deps --locked
node --test scripts/contract-consumers.test.mjs
node scripts/check-contract-consumers.mjs
```

`catalog-schema` 输出当前结构 schema；`catalog-golden` 输出目录和 schema 的规范摘要。golden 变更必须有意审阅，不能以重新生成代替漂移调查。独立 consumer 使用真实公共 example 和固定测试目录，不证明后端/OS/模型能力；全量验证按本仓 `make ci`。

后端边界及固定来源见[对齐说明](../../docs/guides/202609130000-2396-service-catalog.md)。
