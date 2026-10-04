# Agent 通信与内容客户端

本 crate 消费 rss-mdm 服务端产品唯一持有的 Agent V1，提供注册、报告补传、签名任务、可信内容缓存及执行结果交付。只支持当前协议和通信数据库格式；不兼容旧协议、不转换旧执行库，不用测试身份补救生产身份失败。

## 宿主接入

设备 owner 提供可靠 UTC、实际平台/架构、固定 HTTPS origin/tenant、任务签名公钥、独立 HTTPS/设备签发 CA、原生身份 provider 和显式预算。身份 provider 在本机生成不可导出 RSA 设备密钥，CSR 与 TLS 使用同一原生签名器；一次性授权秘密单独保存在 OS 保护存储，同一引用在重试期间不可换值。注册回执不包含 tenant，不能用收到的任务自证身份。

这里的注册是 Agent 与 rss-mdm 服务端之间的设备身份注册。wire 的 `MdmEnrollmentV1` 指操作系统标准 MDM enrollment（由 OS/用户批准的管理注册入口），由 producer 定义；当前客户端明确拒绝该能力，不能以 Agent 注册或任务领取代替。

首次初始化和重开数据库是不同操作，根目录必须已存在且私有。同一通信根只有一个驱动 owner；API 不启动常驻 worker。正常路径为：

1. 注册或读取持久回执；报告先入队，再驱动有界补传。
2. 领取并验签 Offer；确认 Received，下载全部精确材料。
3. agent-service 内部 backend 适配本地计划并检查材料、目标、预算和宿主前提；client 获取并验证 Start。
4. backend 消费当前 Start，通过既有 ExecutionApp 的本地准入和唯一 journal 派发；client 只持久保存精确关联，网络许可不代替本地执行身份校验。
5. 执行 owner 独立驱动 reconcile；backend 准备 journal 结果，client 冻结并发送 HTTP，在通信事务提交后返回不可伪造的 ACK 回执；backend 据此逐事件确认本地结果，再释放关联和无引用缓存。HTTP 等待不持有应用借用。

Offer 最长 60 秒，Received 不续租；Start 最长 15 秒。下载中断保留部分文件；过期后重新 claim，只有新签名仍声明同样的长度和摘要时复用。已经提交的 Start 请求使用原身份重放，但不能延长原许可。进程重启后使用 recover_start，从冻结请求和完整缓存恢复包装，不对过期 Offer 开放首次 Start 或下载。未知本地执行先核实，不重复派发。换 attempt 时原子退休过期 Received 请求及旧缓存关联；任何不确定 Start 或结果结算仍保留并阻断换代。

软件载荷完整保留步骤、检测、精确变体和产物。服务内部 backend 支持绑定为一个冻结本地 program 的有序多步骤，以及独立 detect intent；尚未实现的软件格式、发布源、检测机制或缺失宿主能力会明确拒绝，平台执行仍由原 owner 持有。未进入本地 journal 的 Offer 可由 backend 验证 journal 无执行后，通过 client 的冻结取消与持久回执精确结算，服务端确认后再释放关联；已有本地执行或未知效果继续由 journal 处理。自选软件的 start_user_initiated 只能由已验证的本地交互入口调用，不暴露为普通 UI/AI 透传操作。

client 的 CredentialRedactor 从受保护身份 provider 获取实际一次性授权秘密；backend 在结果进入网络前调用其脱敏能力，秘密列表不对外暴露。stdout/stderr 分别严格消费冻结计划的 UTF-8/UTF-16LE 编码；非法编码和字符截断保持原始 journal 证据，网络诊断使用明确的解码失败标记，质量为 Failed/Truncated，不能阻塞结果交付或补成成功。生产身份、凭据保护和企业装配复用 #2564 的 agent-service；管理员授权的后台任务不增加本地企业审批。缓存文件与原始证据仅供可信 Rust 宿主，不投影到 UI/AI。

首次注册由显式 browser request 或受保护的 GrantImport 元数据及单独秘密驱动。保存原 grant、CSR、公钥引用、证书/激活操作与候选；响应丢失只查询或重放原操作。IssuanceUnknown 保留原 CA 意图等待管理员恢复；ActivationUnknown 保留原候选与激活操作。人工换证保持完整 runtime binding 和 execution journal。撤销、过期、退休证书分别报告，不以新的注册或 Bearer 回退。

## 验证

本 crate 的 acceptance 使用仓内 HTTP/TLS fixture、实际 loopback socket、通信 SQLite 和文件系统验证客户端协议、凭据、材料与失败恢复。服务内部 backend 测试使用同一 HTTP fixture，另以真实通信库和执行 journal 验证执行关联及结果交付。fixture 不证明服务端业务或生产环境验收。

```sh
python3 scripts/build-run.py -- cargo test --locked -p agent-client --test acceptance
```

构建、生成与测试只消费本仓输入及公开依赖，不读取或构建 rss-mdm checkout。完整产品联调由独立验收任务连接已部署服务或固定服务制品，不作为客户端常规 CI 前提。
