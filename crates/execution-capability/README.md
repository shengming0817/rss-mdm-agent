# execution-capability

C06（#2399）：`match_capabilities(&FrozenPlan, &EnvironmentSnapshot, MatchLimits)` 纯计算能力匹配。仅依赖执行契约和静态错误基础库，不探测OS、不创建服务或沙箱。

所有执行要求来自冻结计划，尤其是必填的 `SessionRequirement`；没有单独覆盖参数，也不从发起人的OS登录推断目标会话。逐项检查完整authority/tenant与目标设备、平台、精确解释器（ID/revision/hash）、运行身份、指定用户会话、网络/读写路径限制，以及需要时的子进程限制和沙箱边界。`requireSandbox=false`不取消其它强制约束。

Inventory 明确标记观察覆盖度：命中条目是 Available 或 Blocked；完整清单缺项为 Unsupported，不完整清单缺项为 Unknown。所有inventory都验证重复项和总条目上限，结构错误不会返回部分成功。列表为空不默认为支持。

MatchReport 绑定 plan digest 和 snapshot source/revision，保留稳定顺序的所有必需维度；总体优先级为 Unsupported > Unknown > Blocked > Supported。authority/tenant或目标设备不匹配时，全部维度归 Unknown，不能使用另一设备的能力放行。

Supported 不是授权、执行许可或真实OS支持证据。host 必须验证快照真实性、新鲜度和探测覆盖；隔离能力表示能强制整项计划约束，不只是存在同名API。runner仍负责实际端点、路径解析、权限和隔离强制，C06不证明这些效果。

验证：`cargo test -p execution-capability --locked`。公共示例 `capability-consumer` 接收固定计划 fixture，仅生成明确的测试快照；完整独立消费见[开发指南](../../docs/guides/execution-cores.md)。
