# script-plan

把已校验的目录参数、精确脚本/解释器产物和执行上下文编译为同一个冻结计划；不产生第二份摘要，不解析或执行脚本。

静态解释器 profile 决定 argv，参数保持独立，禁止 shell 拼接或再次 tokenize。秘密仅走环境/受控 stdin 引用，不进入 argv；禁止继承环境及加载控制变量。宿主必须验证 profile 与精确解释器关联、安全物化 artifact slot，并强制 IO 和平台约束。

支持的解释器、参数编码和环境限制由 [src](src/) 持有。编译成功不证明脚本安全或 OS 隔离；能力未知应阻止执行。来源见[执行核心来源](../../docs/reference/execution-cores.md#c10c11-脚本与软件计划)。
