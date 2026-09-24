# A03 Host 来源与改写

本次为 MIT 产品代码重新实现，没有复制历史数据库、执行授权、PR 业务或原生 transcript。来源用于验证机制与边界，实际验收仍以本仓测试和运行结果为准。

| 固定来源                                                                                                                                                                       | 采用及改写                                                                                                  |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------- |
| [Node.js `lib/child_process.js` @ v24.14.1](https://github.com/nodejs/node/blob/v24.14.1/lib/child_process.js) 与同版本 `doc/api/child_process.md`                             | detached POSIX 进程组、明确 stdio 继承与 exit 事件。Host 另核实空组，不把单进程退出或信号成功当作全组退出。 |
| [Cline desktop sidecar architecture @ d48afb3](https://github.com/cline/cline/blob/d48afb3542b07cbb6d17b40d7c536aadc9cbf041/apps/examples/desktop-app/sidecar/ARCHITECTURE.md) | UI 与 runtime 生命周期分离。本产品复用 A04，不复制另一套 UI/聊天协议。                                      |
| Node 官方 [v24.14.1 SHASUMS256](https://nodejs.org/download/release/v24.14.1/SHASUMS256.txt)                                                                                   | macOS arm64 archive 的固定 SHA-256，运行包保留 Node LICENSE 与实际平台标记。                                |

逐文件 owner：`packages/ai-host/src/index.ts` 持有 mailbox 与公共 Host；`channel.ts` 持有有界 IPC；`process.ts` 持有当前进程所有权；`bootstrap.ts` 持有激活前后的隔离边界；`apps/ai-host` 持有可信本地装配。A01 transition / SQLite schema 仍分别由原包持有，没有平行状态库。

PR #1061 修复参考：Node `v24.14.1/lib/child_process.js` 的 timeout/kill 与 exit 分离；[OpenSSH authfile.c](https://github.com/openssh/openssh-portable/blob/master/authfile.c) 的私有材料 mode 检查（本产品额外要求当前 UID 及非 symlink）；[Kubernetes pod_workers.go](https://github.com/kubernetes/kubernetes/blob/master/pkg/kubelet/pod_workers.go) 的独立 worker 终止状态。Host port 与 `deadline.ts` 属本仓实现，未复制上游源码。

## 原生会话与恢复机制参考


ref: OpenAI Codex `codex-rs/app-server-protocol/src/protocol/v2/mod.rs`、`core/src/session/session.rs`，固定 revision 见 [Codex 来源](../reference/codex-adapter.md)。沿用原生 request/turn terminal，不改原生协议：Codex adapter 按 Host 单次 dispatch 的匹配终态结束观察迭代器，释放单读队列；无 request 坐标的会话观察仍由调用预算结束。原生接受 turn 后报告 running，Host 的 steer 仅继承目标 run、独立绑定新请求，避免合法 steer 被状态或旧请求坐标拒绝；取消和回答仍保留原请求坐标。

恢复测试与生产应用使用同一个配置/主体/准入 resolver，只在现有 Store.commit 接缝暂停，不给生产 Store/Host 添加故障开关。Rust 编译 helper 被原有丢回执测试与共同受控测试共享，子进程构建设有硬超时。

ref: oh-my-pi [crates/sandbox/src/runner.rs@2f92f3b5aa2035d21c9f016f9ff6350795e5685a](https://github.com/can1357/oh-my-pi/blob/2f92f3b5aa2035d21c9f016f9ff6350795e5685a/crates/sandbox/src/runner.rs#L69)：借鉴 requested/enforced/missing 的证据表达；这里只记录现有 verifier 的要求、断言与缺口，没有移植 sandbox 平台或扩大 A01。

ref: Node.js [lib/internal/url.js@v24.14.1](https://github.com/nodejs/node/blob/v24.14.1/lib/internal/url.js#L879)：`href` 保留规范化路径，`origin` 仅含协议和主机。端点摘要直接复用传入 adapter 的同一 URL 值。
