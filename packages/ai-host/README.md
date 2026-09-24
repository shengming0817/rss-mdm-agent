# AI Session Host

协调持久会话、命令队列与独立 provider worker，是 AI 数据库的唯一写入者。Rust 执行授权和业务 journal 保持独立。装配与操作见[应用入口](../../apps/ai-host/README.md)，设计见[状态与进程所有权](../../docs/architecture/ai-host.md)。

组合根提供可信配置、worker 模块和独立 launch fence 存储；持久登记成功后才激活 provider。受控工具 verifier 留在 parent，工具参数不能构造调用主体。持久命令账本是唯一队列，内存只负责调度。

接纳与派发意图先持久化，未知结果核实原尝试，不自动重发。产品会话可先于 provider 存在，连接切换等待旧队列完成；用户切换取消模型工作，设备任务保持原 actor。

恢复不能向保存的 PID 发信号或接管未知进程。关闭停止新准入并有界等待，实际 root exit 与空进程组才证明完成；失败保留 fence 和待核实事实，用新预算重试。调用方必须检查关闭结果。

API 与配置边界见 [src](src/)，来源见[参考记录](../../docs/reference/ai-host.md)。
