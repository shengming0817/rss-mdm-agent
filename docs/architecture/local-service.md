# 本机安全服务与私有进程通道

对应 [#2462](https://dev.azure.com/shengming0923/rss/_workitems/edit/2462)。本项扩展到 macOS 26.4 arm64、Windows 11 x64 的实验室候选实现及桌面接线。真实平台验证是独立完成门，不从交叉编译、S1 或 mock 推定完成。

## 权威与信任

管理员固定安装产物、允许查询的 OS 用户与服务账号。服务只有 GetServiceStatus，返回安装实例、构建与 statusOnly 能力。安装实例不是 rss-mdm Device/Registration，也没有业务批准、执行、凭据或 worker 接口。

macOS 以非登录账号运行 LaunchDaemon，NSXPCConnection 对等代码 requirement 由 OS 执行；服务核对连接的 UID、audit session、PID 和受保护产物。Windows 以虚拟服务账号运行本机 Named Pipe，显式 DACL 不向客户端授予创建 pipe instance 的权利；客户端验证服务 SID/session/固定映像，服务在读取固定握手前缀后短暂 impersonate 核对真实进程及用户 token，返回后立即 RevertToSelf。PID 查询句柄保留到请求结束。任何证据缺失均拒绝。

保护对象是独立恶意进程、跨用户接入、假端点、错误或替换产物和旧消息。可信桌面已经被注入、本机管理员或内核失陷不在本批保证内。同用户 token、正文、路径或 PID 单独不构成身份。安装清单没有秘密，受管理员写权限保护。

## 单次查询

完成身份核验后生成 256-bit 随机 challenge，只存在于当前连接。查询必须携带同一个 challenge 和唯一版本/方法，五秒单调时钟预算覆盖接入与处理。先消费再处理，包括非法请求；第二次请求没有授权。客户端核对回包 challenge 和安装身份。

无需长期 session、重放表、服务 epoch、续租或授权票据。停止服务使连接失效，重启不恢复 challenge。管理员撤销先停服及关闭连接，再原子替换清单，失败保持停服。新查询重新认证。认证结果与进程终止、业务任务成功是不同事实。

| 候选机制 | 决定 | 证明范围 |
| --- | --- | --- |
| HMAC | 不采用 | 本机直接 OS IPC，双方身份与内核通道；不覆盖被接管的可信端主动改写 |
| 协议 nonce | 一次性 challenge | 重复、跨连接、过期和服务重启的旧请求 |
| 凭据票据 | 不采用 | 服务不接收、存储、解密、转发 AI 密钥 |
| worker grant | 不采用 | worker 无服务查询权；不声称 provider 密钥具有服务端撤权 |

篡改验收分别证明非法进程不能建立获准通道，以及非法字段/方法/版本/challenge 被拒绝。业务 command 幂等回执不能替代 IPC 重放证明。

## 私有 AI 进程

Native 唯一拥有 Host，Host 唯一拥有逻辑 worker。全部使用私有继承 stdin/stdout，stderr 为诊断。V1 帧是 RSS + 版本字节 1、闭集 lane 字节、四字节 big-endian 长度和有界 payload。Native 固定 native/execution，worker 固定 control/tools/events；各 payload 的声明源不合并。旧 fd3–5 入口删除。

单 reader 只分帧分发，单 writer 优先控制队列，各 lane 独立限额；饱和或非法帧关闭连接。物理管道堵塞由 owner 的 OS 终止预算兜底。控制队列优先不等于任意输出期间都能及时交付。

固定 launcher 从自身目录读取 manifest，校验 Node/bootstrap 摘要，不接收任意命令或入口路径。macOS 使用进程组，Windows 在恢复初始线程前加入 Job Object，禁止 breakaway，并启用 kill-on-close。launcher 监视 Host 的父进程生命周期。普通权限私有文件工具只检查当前用户的文件/ACL，不提供提权能力。

launch fence 保存 namespace、launchId、provider artifact、runtimeDigest 和带平台标签的 scope；保留预留/登记两阶段，登记前禁止 activation。OS handle 不持久化。恢复只能只读确认范围已不存在；未知、权限不足、仍存活都阻断，禁止凭保存 PID/job 名称终止进程。

## 直接替换

Host readiness protocol 3、私有承载 V1、AI SQLite schema 5 协同交付。AI 业务 wire V5 的未变部分保留原语义。旧启动入口、承载及 SQLite 格式拒绝，没有双读、迁移、alias 或自动降级。旧库原样保留，由实验室操作者选择新目录，不能删除旧库来冒充恢复成功。

macOS 主密钥仍在 Keychain，Windows 采用当前用户 DPAPI 与专属 ACL，输入只返回 Native 保存操作。主密钥不进入 worker/WebView。官方 Codex/Claude 登录仍由官方组件负责。
