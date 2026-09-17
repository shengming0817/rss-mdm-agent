# script-plan

C10 原生脚本纯计划核心。`compile(ScriptPlanInput, &PlanLimits)` 将已解析的 C03 参数、精确脚本/解释器产物及完整执行上下文编译为 C01 `FrozenPlan`。没有独立 ScriptLaunchPlan、摘要、授权、脚本内容解析或 OS 执行。

## 编译规则

- PowerShell 7 使用固定 `native-pwsh7-file@1` profile：`-NoLogo -NoProfile -NonInteractive -File <artifact>`。字符串/安全整数编码为两个独立 argv（`-Name:`、原始值）；冒号使解析器的 pendingParameter 分支优先消费下一项，保留 `-1`、`-flag`、`true` 等字符串。布尔量使用单项 `-Name:$true` / `-Name:$false`。不拼接 `-Command`，不支持 Windows PowerShell 5.1。
- POSIX sh 使用 `native-posix-sh-file@1`；Bash 使用 `native-bash-file@1` 与 `--noprofile --norc`。只支持 macOS/Linux、无 BOM UTF-8 脚本与位置参数。参数按绑定顺序形成独立 argv，特殊字符原样保存；不生成 shell quoting。
- 每个输入参数必须且只能绑定一次，允许标量字符串、布尔值及 JSON 安全整数；不展开列表/对象，不复刻 C03 schema、默认值或表达式。PowerShell 命名参数只接受有界 ASCII 标识符，拒绝大小写重复。
- 秘密只通过环境引用或受控 stdin 引用传入；不允许 argv 秘密或字面 stdin。环境无继承；Unix profile 拒绝 `BASH_ENV`、`ENV`、`SHELLOPTS`、`BASHOPTS`，环境键类型同时排除导出的 Bash 函数语法。所有 profile 还拒绝 LD_/DYLD_/DOTNET_/COMPLUS_/CORECLR_/COR_ 加载控制命名空间及 GCONV_PATH、GLIBC_TUNABLES、PSModulePath（含大小写变体），模板和参数绑定环境共用同一检查。
- 输入、输出编码、stdin 上限、runAs、交互 session、网络/路径/子进程限制、总预算和有效期全部进入 C01 摘要。宿主须验证 profile 与精确解释器的关联，验证并安全物化脚本路径；artifact slot 只能替换为该计划产物的绝对路径，不能重新 tokenize。

这些规则不证明脚本安全，也不强制 OS 隔离。环境清理、文件/解释器 hash 验证、编码、原始输出保存、无损失败诊断、管道关闭及预算计量由后续 runner 负责；C06 不支持或未知的能力必须阻止执行。

公开错误使用无输入值的静态原因，区分参数缺失/重复/未消费、命名非法/重复、环境冲突与 profile 不支持的目标；消费者无需复刻编译校验来定位错误类别。

## 验证

```sh
cargo test -p script-plan --locked
cargo run -p script-plan --example script-plan-consumer --locked
```

示例使用显式测试 authority，无真实 shell/平台证据。公共 API 同时由隔离 consumer 运行，规则见[契约开发](../../docs/guides/contracts-development.md)。固定一手源码与差异见[执行核心来源](../../docs/reference/execution-cores.md#c10c11-脚本与软件计划)。
