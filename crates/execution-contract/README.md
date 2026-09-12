# execution-contract

本地执行值类型、预算校验和不可变计划摘要。该 crate 无数据库、UI、OS、AI 或 MCP 依赖，不定义 rss-mdm 拥有的远程 Agent wire。

## 信任与数据流

`decode_plan(bytes, limits)` → `PlanSpec` → `FrozenPlan::freeze(spec, limits)` → 只读计划与摘要。

`ExecutionRequest` 是意图，`PlanSpec` 嵌入原请求并且唯一持有解析后的执行描述。`FrozenPlan` 的字段私有且没有 `Deserialize`；存储读取须重新解码、冻结，再用 `matches_digest` 比对原摘要。改变计划需要取得副本并重新冻结。结构校验、摘要匹配都不证明 actor、authority、批准或 Evidence 真实有效；不存在可信主体、批准签发或执行 permit API。

所有可反序列化类型都是 DTO；应用接收外部字节使用有界 decoder。直接构造/反序列化 `PlanSpec` 不能绕过 freeze 的语义校验。参数与环境 map、动态 literal 也拒绝重复键。秘密使用带 revision 的 `InputValue::Secret`，host 不得将长期凭据伪装成 literal/argv；运行时解析、访问授权及缓存替换检测属于后续 owner。`Debug` 与公开错误不回显参数、argv 或 provider 输入。

## V1 摘要

摘要为 `SHA-256(b"rss-mdm-agent/execution-plan/v1\0" || JCS(PlanSpec))`，输出小写 64 位 hex。JCS 使用 `serde_json_canonicalizer =0.3.2`；Rust 声明派生唯一序列化形态。冻结对象保存 JCS 规范字节重新解析后的值，执行方不得继续使用冻结前的原始输入。

所有计划字段（包括 request/plan ID）都参与摘要：actor、initiator、authority/tenant、delegation、device/用户目标、operation/resource、产物/解释器版本与摘要、参数与秘密版本引用、argv/cwd/env、runAs、constraints、budget、validity、policy。实际批准和执行后证据另行引用该摘要，避免循环。键顺序/空白及等价数值表示不影响摘要；数组顺序不变；字符串不作 Unicode/路径规范化。

数值采用 JSON/IEEE-754 语义；整数绝对值不得超过 2^53−1，超界整数（含整数值的浮点表示）、NaN、Infinity 均拒绝。需要更高精度的业务值用目录 schema 明确声明的字符串传递。fixture 的 SHA-256 向量通过独立 Python JSON 排序及 hashlib 生成，使用 ASCII 键和安全整数；测试不调用被测实现生成 expected。

## 限制与执行描述

`PlanLimits` 是 host 必填约束，无 Default/unlimited。字节数在解析前检查；字符串、集合、深度、节点数和数值在规范化前检查。深度最多 64（JSON 容器/值层），节点包含值，集合上限对每个 array/object 生效。`max_input_bytes` 同时限制输入及规范编码长度。预算 totalTimeoutMs/totalOutputBytes/maxAttempts 必须大于零且不超过 host 上限。前两项是整个计划的累计上限：时间从首次 attempt 开始计，包括其后的重试、退避和等待，首次执行前的批准等待不计；输出计入全部 attempts 的 stdout/stderr/result 字节，包括丢弃或截断的字节。maxAttempts 不能乘大或重置这两个上限；实际计量、取消及持久化由后续 lifecycle/runner owner 强制。

有效期使用 UTC Unix 毫秒，要求开始早于截止；当前时间、时钟可信性、委托约束和撤销状态由后续授权 owner 验证。这里没有系统时钟或时效放行逻辑。

`Target` 明确 device/platform 和 device/user scope；`RunAs` 单独描述目标 OS 身份，两者平台须一致。产物/解释器必须携带资源 ID、revision 和 SHA-256；本 crate 不解析、下载或验证文件。cwd/readPaths/writePaths 要求绝对路径，无 NUL 或 `.`/`..` 分量；真实路径、符号链接、OS 能力和 sandbox 强制由平台 owner 验证。argv 是独立参数，禁止 NUL，不拼接 shell。env 是显式映射，无环境继承默认。

`NetworkAccess::Denied` 表示禁止网络；Allowlist 是精确目标声明，空集合和通配符拒绝。runner 必须支持并强制所有 constraints，不能因声明存在就视作隔离已落实。

`AuditEvent` 持有最小完整关联字段，device 从 target 获取，不复制第二个 device 字段。只保存引用和 reason code，不包含参数、脚本文本或输出。`Decision::Observed` 必须携带 AttemptId 和私有构造的非空 EvidenceRefs；此前的裁决事件没有虚构的执行 evidence。`decode_audit` 和程序构造后的 `AuditEvent::validate` 共用校验，拒绝测试 authority 携带真实进程/状态 evidence 类别；其它 evidence 仍只是未验证记录，不能据 DTO 推导 Applied/Converged。准入审计与 intent 的原子持久化属于 C18/C19。

## 验证

- `cargo test -p execution-contract --locked`
- `cargo run -p execution-contract --example execution-consumer --locked -- crates/execution-contract/tests/fixtures/plan.json crates/execution-contract/tests/fixtures/plan.sha256`
- `cargo run -p execution-contract --example execution-schema --locked` 输出 Rust 派生 schema，供有意更新 golden 时使用。

Schema 固定 Draft 2020-12，描述 V1 结构、枚举、ID 和摘要格式；动态预算、平台一致性等由 Rust 校验负责。未知字段/版本/变体拒绝，不提供旧格式 reader。`tests/fixtures/schemas.json` 是派生产物，不能手工作为第二声明源维护。

有界 decoder 保留自身构造器的 Version/Value 错误类别，JSON 损坏/结构错误返回 Encoding；不通过任意 provider 错误正文推断类别，错误不附带原输入。
