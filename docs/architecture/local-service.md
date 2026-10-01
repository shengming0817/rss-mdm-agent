# 唯一生产执行服务与开发装配

apps/agent-service 是唯一生产系统组合根，持有设备身份、执行 journal、恢复与结果送达。桌面和 AI Host 只消费认证 IPC，用户 helper 仅执行原登录会话的物理动作，不拥有注册、业务授权或账本。

installation-security 持有管理员固定映像、摘要/代码身份、受保护配置与目录检查。系统 IPC 继续使用真实 OS 对等身份及专属操作认证；本机 IPC V6 拒绝旧格式，不协商或回退。远程 Agent wire 和 helper 协议由各自原 owner 持有。

可信配置/pins 通过后，同一个监听器装配就绪执行 owner 或只读未就绪诊断。只有能证明从未初始化才报告待注册；损坏、身份冲突或未知残留不触发初始化、重置或重新注册。状态连接成功不表示任务获准或效果已发生。

开发 fixture 在启动前选择，复用 tests/assistant、Rust 生成投影和产品 App。debug-only 的 dev-fixture 主程序只注册外观及装配标识，不启动生产账户、AI Host、钥匙串、服务 IPC 或 journal。App 显式消费该 owner 提供的身份、Host、状态及执行 ports，缺失任一项即拒绝启动；确认与撤销进入原请求及 revision。浏览器、原生人工开发和视觉验收使用同一个 fixture owner；正式构建禁止该装配，连接错误不会切换模式。

固定候选部署、显式刷新和真实平台边界见[实验室操作](../guides/local-service-lab.md)。历史 #2462 的一次性状态服务已退出；其必要 OS 保护机制保留，双平台安全验收由 #2559 消费最终统一候选。
