# software-plan

C11 软件安装计划与核实决策纯核心。`decide(&SoftwareIntent, &PlanningSnapshot, PlanningLimits)` 返回上下文绑定的单一下一步：`Satisfied / Detect / Wait / RequireRestart / Mutate / Blocked`。没有安装器、依赖求解器、执行状态机、授权平台、第二份执行摘要或 C11→FrozenPlan 转换。

## 输入与信任

宿主认证并校验事实来源、时效和一致性；核心核对 authority/tenant、device/platform/user、policy revision、manager/source revision 与包坐标的精确绑定。`Ready` 只表示没有已知临时阻碍，执行前仍须获取资源及包管理器锁，重新验证事实，并通过 C07/C08/C09。私有输出类型不是 execution permit。

包名、版本、架构和变体采用有界不透明文本，不用 SemVer，不做大小写折叠、版本范围求解或 latest 回退；来源 owner 必须先解析浮动别名并绑定精确 payload。不同版本的顺序由带 comparator revision 和精确操作数的生态比较结果提供；缺失/不可比较分别阻塞。完全相同文本可直接判等；生态判等保留原始版本。

`Detection::Present/Absent` 必须来自独立状态观察，不能将 ProcessExited 升格为状态证据。Test authority 仅接受 TestResult，生产 authority 拒绝 TestResult；引用真实性仍由宿主核验。快照及决策没有 wire Deserialize 接口。

## 决策约束

- 无可用观察、未知安装结果、取消后的未知效果、变更完成及重启后均要求独立 Detect；检测完成但仍歧义则 Blocked，不能盲目重试。
- 观察已满足目标时保留原证据和 ownership，不自动接管用户软件。未知 ownership 阻止变更；修改用户已有软件须显式允许。
- required 禁止目标缺席；移除与卸载前置升级要求移除权限且依赖引用已确认 Unused。升级保留 `UninstallThenInstall` 语义，同时要求 Install/Uninstall 能力。降级需单独许可。
- 未声明的依赖影响阻塞，已声明影响须符合管理约束；核心不生成第二份依赖图或跨包事务。
- 自动重启不可协调，始终阻塞；可能请求重启须明确允许。已观察的重启要求返回 RequireRestart，实际重启另行授权，重启后重新检测。
- 临时资源/包管理器占用、维护窗口和应用占用返回 Wait。每次 Mutate 都携带精确 installer、变更前版本与强制 post_detection 目标，进程退出不构成 Satisfied。

结构诊断区分 authority、target、policy、package、installer manager、用户平台及比较上下文/操作数不一致，均不携带输入值。业务禁止使用 Blocked。每次事实变化需新建不可变 snapshot identity/revision；示例分别保留安装前、未知结果、独立观察的引用，并区分 package payload 与 installer binary。

## 验证

```sh
cargo test -p software-plan --locked
cargo run -p software-plan --example software-plan-consumer --locked
```

测试覆盖决策表、跨上下文替换、证据类别、来源约束与独立消费。所有场景为合成事实，不提供真实安装或 T3 证据。[源码对标](../../docs/reference/execution-cores.md#c10c11-脚本与软件计划)记录所采纳与未采纳的机制。
