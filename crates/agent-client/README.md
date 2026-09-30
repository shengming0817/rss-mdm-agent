# Agent 通信与内容客户端

本 crate 消费 rss-mdm 唯一持有的 Agent V4，提供注册、报告补传、签名任务、可信内容缓存及执行结果交付。只支持当前协议和通信数据库格式；不兼容旧协议、不转换旧执行库，不用测试身份补救生产身份失败。

## 宿主接入

设备 owner 提供可靠 UTC、实际平台/架构、固定 HTTPS origin/tenant、可信公钥、受保护秘密 provider 和显式预算。秘密 provider 在首次注册前生成并保护随机 credential，同一引用在重试期间不可换值。注册回执不包含 tenant，不能用收到的任务自证身份。

首次初始化和重开数据库是不同操作，根目录必须已存在且私有。同一通信根只有一个驱动 owner；API 不启动常驻 worker。正常路径为：

1. 注册或读取持久回执；报告先入队，再驱动有界补传。
2. 领取并验签 Offer；确认 Received，下载全部精确材料。
3. 可信宿主适配本地计划，调用 ExecutionBridge::prepare 检查材料、目标、预算和当前宿主前提，再请求 Start。
4. bridge 消费当前 Start，通过既有 ExecutionApp 的本地准入和唯一 journal 派发；网络许可不代替本地身份或批准。
5. 执行 owner 驱动 reconcile，bridge 投递 journal 证据；服务端持久确认后才逐事件确认本地结果，最终释放关联和无引用缓存。

Offer 最长 60 秒，Received 不续租；Start 最长 15 秒。下载中断保留部分文件；过期后重新 claim，只有新签名仍声明同样的长度和摘要时复用。已经提交的 Start 请求使用原身份重放，但不能延长原许可。未知本地执行先核实，不重复派发。

软件载荷完整保留步骤、检测、精确变体和产物。当前具体 ExecutionBridge 接受可表达为一个本地计划的任务；复合步骤、detect-only 或缺少宿主能力会明确阻塞，平台/复合适配仍由其 owner 持有。自选软件的 start_user_initiated 只能由已验证的本地交互入口调用，不暴露为普通 UI/AI 透传操作。

OutputPolicy 必须由可信宿主提供，在结果进入网络前保护输出秘密。生产身份、凭据保护、批准和企业装配归 #2564；当前默认生产执行仍不启用。缓存文件与原始证据仅供可信 Rust 宿主，不投影到 UI/AI。

## 验证

本 crate 的 acceptance 使用实际 loopback socket、SQLite 和文件系统，以及明确的 Test host/runner。live_mdm 是同一产品测试入口，连接正式 rss-mdm serve 与真实 PG，不是另一个消费者工程。

准备隔离服务环境、正式 HTTPS 网关、内容目录及任务签名配置后，在私有配置文件中提供 origin、tenant、ca_file、admin_password_file、key_id 和 public_key。配置不入库；管理员须拥有授权管理权，测试经公开 API 配置业务授权、资源、Scope 和 Policy。真实联调操作只允许针对可丢弃测试环境。

```sh
python3 scripts/build-run.py -- cargo test --locked -p agent-client --test acceptance
AGENT_LIVE_CONFIG=/private/test-environment/agent-live.json \
  python3 scripts/build-run.py -- cargo test --locked -p agent-client --test live_mdm -- --ignored
```

缺少 live 配置或依赖会失败，不按跳过解释为通过。受控测试结果不证明真实 Windows/macOS 安装、生产身份、签名包或 T3 成立。

## 来源

直接消费 producer 的验证、编码与闭合 wire。HTTP 行为对标已读取的 reqwest 0.13.5 src/async_impl/client.rs；资源文件复用本仓 native-process::private_storage，结果持久化复用 execution-sqlite。
