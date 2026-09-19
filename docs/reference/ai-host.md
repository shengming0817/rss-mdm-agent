# A03 Host 来源与改写

本次为 MIT 产品代码重新实现，没有复制历史数据库、执行授权、PR 业务或原生 transcript。来源用于验证机制与边界，实际验收仍以本仓测试和绑定源码 SHA 的产物记录为准。

| 固定来源                                                                                                                                                                       | 采用及改写                                                                                                  |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------- |
| [Node.js `lib/child_process.js` @ v24.14.1](https://github.com/nodejs/node/blob/v24.14.1/lib/child_process.js) 与同版本 `doc/api/child_process.md`                             | detached POSIX 进程组、明确 stdio 继承与 exit 事件。Host 另核实空组，不把单进程退出或信号成功当作全组退出。 |
| [Cline desktop sidecar architecture @ d48afb3](https://github.com/cline/cline/blob/d48afb3542b07cbb6d17b40d7c536aadc9cbf041/apps/examples/desktop-app/sidecar/ARCHITECTURE.md) | UI 与 runtime 生命周期分离。本产品复用 A04，不复制另一套 UI/聊天协议。                                      |
| Node 官方 [v24.14.1 SHASUMS256](https://nodejs.org/download/release/v24.14.1/SHASUMS256.txt)                                                                                   | macOS arm64 archive 的固定 SHA-256，运行包保留 Node LICENSE 与实际平台标记。                                |

逐文件 owner：`packages/ai-host/src/index.ts` 持有 mailbox 与公共 Host；`channel.ts` 持有有界 IPC；`process.ts` 持有当前进程所有权；`bootstrap.ts` 持有激活前后的隔离边界；`apps/ai-host` 持有可信本地装配。A01 transition / SQLite schema 仍分别由原包持有，没有平行状态库。

PR #1061 修复参考：Node `v24.14.1/lib/child_process.js` 的 timeout/kill 与 exit 分离；[OpenSSH authfile.c](https://github.com/openssh/openssh-portable/blob/master/authfile.c) 的私有材料 mode 检查（本产品额外要求当前 UID 及非 symlink）；[Kubernetes pod_workers.go](https://github.com/kubernetes/kubernetes/blob/master/pkg/kubelet/pod_workers.go) 的独立 worker 终止状态。Host port 与 `deadline.ts` 属本仓实现，未复制上游源码。
