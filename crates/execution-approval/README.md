# execution-approval

C08 只判断精确批准是否适用并生成消费意图，不签发批准、不写数据库、不调用执行器。运行依赖为 C07 execution-admission 与 C01 execution-contract；不引入 UI、OS、数据库或 JWT 平台。

## 公共入口与信任边界

~~~rust,ignore
let decision = execution_approval::evaluate(
    &frozen_plan, &admission_decision, &attempt_id, &profile_bindings,
    &trusted_verifier, limits,
);
~~~

直接消费不可反序列化的 C07 AdmissionDecision；先核对完整计划 ID/摘要和授权上下文。Allowed 返回 NotRequired，Denied 返回拒绝，二者都不调用批准验证器。只有 ApprovalRequired 验证全部必需 profile，不接受调用方重建的子集。人工与 AI 共用规则，人工执行并非一律审批。

ProfileApproval 只是 profile 到批准记录的精确版本引用。可信 ApprovalVerifier 批量核验签名或受保护来源、批准者在该 authority/tenant 与政策下的批准权限、计划完整范围、当前撤销状态、可靠时间和一致快照。返回的 ApprovalFacts/ApprovalRecord 不支持 Deserialize，普通交互或模型返回的 DTO 不能绕过验证接缝；恶意宿主仍可实现不可信 verifier，纯库不抵抗同进程宿主或本机管理员。

计划 ID 与 C01 完整规范摘要绑定 actor、initiator、delegation、tenant/device/target、run-as、产物、参数、约束、预算与政策；不再复制这些字段形成第二套批准范围。旧摘要、错误命名空间、过期/撤销/未知状态、漏项与混合记录版本均拒绝。测试 verifier 仅存在于测试/示例，不交付生产签发或加密 adapter。

## 消费与后续接线

每次新 attempt 对每个不同批准记录消费一次；同一记录覆盖多个必需 profile 只扣一次。可预授多次使用，所有 profile 必须同时满足。ConsumptionIntent 为私有构造、不可反序列化的候选 CAS，绑定计划、attempt、记录版本、期望已用次数/消费 revision、可信验证 revision 和有效截止时间。重复计算没有写入副作用。

C19 每次新 attempt 重新执行能力/授权/批准检查。C18 必须在同一受保护事务内：

1. 核对 authority/plan/attempt 与批准决定、生命周期 BeginAttempt 候选的绑定，复核有效期和当前 trust/policy/revocation revision。
2. 校验事件/attempt 历史唯一性、生命周期 revision、每个记录版本与消费计数 CAS。
3. 原子提交全部消费、执行 intent、生命周期快照和裁决审计；成功后才允许派发 runner。

事务失败不消费也不开始总时钟；成功接纳即增加 attempt/批准使用并开始首尝试总时钟，即使随后失败或未派发也不自动退还。同一 attempt 重放必须读取已提交结果，不能重新扣减；新 attempt 必须取得新决定。快照的新鲜期限不是可以忽略撤销/政策变化的缓存授权。

## 验证

~~~sh
cargo test -p execution-approval --locked
cargo run -p execution-approval --example approval-consumer -- crates/execution-contract/tests/fixtures/plan.json
~~~

行为测试覆盖 Allowed/Denied、完整 profile、去重、多记录失败原子性、签名失败、记录/计划/命名空间绑定、有效期、耗尽与幂等。跨 C08/C09 的内存事务模型覆盖失败不扣减/不启动预算、并发候选冲突和同 attempt 重放，不替代 C18 SQLite 并发/崩溃 T2。独立源码消费由仓库 consumer 清单执行；不是 registry 发布验收。来源见[执行核心来源](../../docs/reference/execution-cores.md)。
