# 唯一生产执行服务与开发装配

apps/agent-service 是唯一生产系统组合根，持有设备身份、执行 journal、恢复与结果送达。桌面和 AI Host 只消费认证 IPC，用户 helper 仅执行原登录会话的物理动作，不拥有注册、业务授权或账本。

installation-security 持有管理员固定映像、摘要/代码身份、受保护配置与目录检查。系统 IPC 继续使用真实 OS 对等身份及专属操作认证；本机 IPC 使用[当前版本](../guides/local-service-lab.md)，拒绝旧格式，不协商或回退。远程 Agent wire 和 helper 协议由各自原 owner 持有。

企业能力通过正式 resolve 按 ID、稳定工具名或别名精确读取；目录分页只用于发现。新调用读取当前配置，已受理 Run 保持原材料和结果关联。Received 持久 ACK 后材料准备沿原任务继续；Start 只限制首次派发，后台恢复复用原请求与 journal，不重新批准或盲目重跑。

可信配置/pins 通过后，同一个监听器装配就绪执行 owner 或只读未就绪诊断。只有能证明从未初始化才报告待注册；损坏、身份冲突或未知残留不触发初始化、重置或重新注册。状态连接成功不表示任务获准或效果已发生。

开发 fixture 在启动前选择，复用 tests/assistant、Rust 生成投影和产品 App。debug-only 的 dev-fixture 主程序只注册外观及装配标识，不启动生产账户、AI Host、钥匙串、服务 IPC 或 journal。App 显式消费该 owner 提供的身份、Host、状态及执行 ports，缺失任一项即拒绝启动；确认与撤销进入原请求及 revision。浏览器、原生人工开发和视觉验收使用同一个 fixture owner；正式构建禁止该装配，连接错误不会切换模式。

固定候选部署、显式刷新和真实平台边界见[实验室操作](../guides/local-service-lab.md)。历史 #2462 的一次性状态服务已退出；其必要 OS 保护机制保留，双平台安全验收由 #2559 消费最终统一候选。

私有数据保护由 platform-private-storage 统一持有，Rust 与 Node 消费同一原生机制。
platform-credentials 仅持有 OS 凭据机制；设备秘密命名空间与不可覆盖规则属于 agent-service，个人主密钥绑定属于桌面。
安装完整性与私有数据保密采用不同策略；native-process 仅持有进程生命周期。
SQLite 仍按受保护路径打开，不承诺所有 I/O 使用预先检查的文件句柄，也不提供同用户应用沙箱。

本地采集数据由同一服务提供 `query_directory`、`data_read`、`query_submit`、`query_status`、`task_control`。
Native 的 `nativeDataPort` 与 AI 的 `execution_query_*` / `execution_data_read` 使用同一 Rust 契约。
目录默认读取本地；只有显式设置 `refreshPolicies` 才刷新已发布策略。读取结果从不启动采集。
提交使用稳定 UUID，响应丢失后查询原请求；同一登录中的相同查询仍未结束时返回 `inProgress`。

Agent 登记有限的 OS、CPU/内存、磁盘/电源、网络和软件只读配方，复用 NativeCollection 工作器、执行准入与 journal。
登记配方不创建周期策略或调度任务，后台未周期覆盖时仍可手动查询。当前映射版本为 1，按闭合配方、契约版本和范围识别；
查询标签与采样预算不会把同类周期数据和手动数据拆成不同事实。安装实例、PKG 收据和包管理记录保留不同类别。
任意 WMI、注册表和未知原生类型没有默认本地执行许可；已发布脚本/osquery 沿原 Policy 权限、风险和确认流程处理。
原始进程诊断不进入数据接口，脚本结果仅缓存原 Policy 状态接口授权返回的结构化内容，未知标准映射不冒充资产字段。

最小投影保存在现有通信数据库的专属命名空间，按租户、设备、注册世代隔离；不修改执行数据库的 schema。
普通采集的全部子查询投影、请求完成凭据和恢复输入清理在同一事务提交，部分失败回滚后可恢复；保存可信定义后才允许材料回收；当前格式历史按有界 journal 页恢复，定义已被清理时不推测重建。
周期结果在普通完成与关闭路径写入本地投影，上传 ACK 只影响原传输队列。周期标准字段使用 spool 已归一化的同一份观测，不重复计算计数器。缓存错误不改变自动采样及上报语义。
每个投影最多保留约 256 KiB 结构化行，实际读取页限制为 128 行和约 48 KiB，截断及不完整结果明确标记。
总缓存不超过既定 cache/database 预算的四分之一且最多 16 MiB；容量不足先回收旧结果投影，保留请求身份和可信定义。
手动查询优先保留未结束请求，并把可回收的历史控制在总数不超过 32 个（较小缓存预算会相应缩小）；超出保留范围后须重新读取当前查询；执行 journal 的原请求不会因此重放。
快照游标绑定内容摘要，最新结果变化时旧页返回冲突，调用方须从第一页重读。结果质量、采集时间、范围和完整性独立表达。

查询目录提供原 Policy 精确引用及仅含 Input 参数的安全 schema，与现有 MCP 动态 Policy 工具使用同一 Rust 投影。
Policy 查询身份绑定配方、模板版本和真实 OS 账户，不包含登录 incarnation；已授权结果和安全读取元数据原子持久化，重启后同账户可离线读取。新提交仍要求当前 native context 的发布授权，短期发布元数据按 context 清理；未引用定义和已退出策略有界保留，未结束请求及当前登记配方受到保护。
macOS 用户数据读取同时校验 UID 与 GeneratedUID；Native 执行准入与撤销使用同一 epoch writer fence。

目录的 `domains` 是模块选择查询的稳定语义键；单值字段使用 `hardware.os.*`、`hardware.cpu.*`、`hardware.memory.*` 等统一命名。
Windows 的 WMI/注册表列在服务中映射到这些键，磁盘列表统一为 `id/model/size_bytes`；页面无需按平台重新映射物理列。
