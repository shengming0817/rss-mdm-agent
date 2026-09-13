# execution-admission

C07（#2400）：唯一确定性执行授权核心。`decide(&FrozenPlan, &impl AuthorityVerifier, AdmissionLimits)` 返回不可直接构造/反序列化的 AdmissionDecision，区分 Allowed、Denied、ApprovalRequired；不返回执行permit、不签发批准、不修改计数或journal。

## 信任入口

`AuthorityVerifier` 是宿主的可信接缝，不是JSON解码器。它必须独立认证actor/authority，验证OS/provider来源绑定、委托签发者当前权限/接受者/范围，读取受保护政策，并验证时钟与撤销新鲜度。缺失或失败返回闭合 VerificationError，核心统一拒绝。

AuthorityFacts是该port的Rust输出，无Deserialize，也没有`decide(plan, facts)`入口。运行时VerifiedContext只能由核心调用verifier创建，字段私有。不能将传入FrozenPlan回显成allow规则；freeze只证明内容规范化。Rust接口不抵抗恶意或被攻陷的同进程宿主，生产身份/进程隔离接线由C19及后续平台负责。本仓示例仅含显式TestAuthority，不提供生产验证器。

## 精确范围与组合

规则模板复用FrozenPlan中的规范值，不计算第二套摘要。精确比较authority/tenant、actor、动作/资源revision、目标设备/用户、runAs、SessionRequirement、完整launch、参数和constraints。没有目录通配符、路径前缀、参数表达式或约束偏序推理。改变约束也须匹配另一个明确授权范围。

模板request/plan相关ID、initiator不参与权限匹配；origin由verifier单独认证，human/AI/policy不能改变actor权限上限。委托引用单独与已验证委托一致性核对；其精确scope、policy、时间和预算再与actor基础权限取交集，委托自身不产生allow。模板里的delegation字段不能替代这个验证。

计划有效期须包含于主体、授权规则和委托窗口，当前可信时间必须落在计划窗口内。时间为UTC Unix ms，结束排他。预算按计划整体的时间、输出和attempts逐维比较；不能拼接多条规则的部分预算，也不能按attempts重新补给。实际累计消耗由生命周期/持久层拥有。

当前有效的精确Deny优先，并且不能通过超出该拒绝模板的预算或截止来逃避拒绝。其它规则须完整满足scope、预算和窗口才匹配。其次收集全部匹配的ApprovalRequired profiles（排序去重）；最后才是Allow。无匹配、重复规则ID、policy不一致、验证失败全部Denied。

批准要求只是对精确计划的附条件资格，不是已批准；普通拒绝不能靠用户点击升级为批准要求。结果绑定plan ID/digest、policy、delegation和稳定原因码，包含适用rule IDs；拒绝结果中的计划引用仍只是被评估的声明。C08验证批准适用性，C18原子消耗，C19实施其余执行准入。

## 验证

`cargo test -p execution-admission --locked`。公共示例`admission-consumer`只接收测试authority计划并验证allow/approval/deny及载荷替换拒绝，无生产认证或真实执行。完整独立消费见[开发指南](../../docs/guides/execution-cores.md)。
