# ACP–A2UI 开发

接入由 [ai-access](../../packages/ai-access/README.md)、[ai-client](../../packages/ai-client/README.md)和 [ai-ui-bridge](../../packages/ai-ui-bridge/README.md) 提供。产品 wire 由唯一 schema 生成。

1. 可信组合根建立 access service，绑定认证后的 caller 与官方 SDK stream，不能从请求正文获得身份。
2. 客户端协商、创建和恢复会话；普通 ACP prompt 等待终态，产品扩展另行表达接纳与附着。
3. React 挂载 RuntimeSurface，显式提供应用级 SurfaceDrafts 与桌面节点 DOM owner；owner 通过 render 更新官方节点，通过 dispose 释放 DOM。问答和操作模板共用官方 A2UI core，不转换成另一套协议树。普通非秘密输入与未确认回答的原 command identity 保存在应用 owner，renderer 只持有显示、订阅和计时资源；稳定内容仍来自客户端投影。
4. 收到 resync_required 后显式 restore，先收齐同一水位历史再替换投影。分页 token 只定位读视图，不授予身份。

标准权限使用实时回调；恢复历史不能重建原生回调。断开、卸载和取消分别操作，不自动取消执行。期限在提交时再次检查，竞争失败方不能重试，首答方丢回执仍使用原命令。

```sh
pnpm test:ai-access
pnpm check:assistant
```

浏览器配置与产品 fixture 见[桌面指南](desktop-development.md)。内存 Host/Store 仅验证接入语义，不证明持久回执、真实 provider 或 OS 隔离。官方 processor 异常先使卡片失效，再显式恢复；逐条内存更新不等于事务。

调用方必须传播 owner cancellation 并有界等待；关闭超时表示仍有未结算资源，不能据此宣称 provider 终止。公开错误使用闭合分类，不回显底层 payload 或 cause。协议与 renderer 来源见[AI Runtime 来源](../reference/ai-runtime.md)。

操作模板使用 ai-contract 的独立展示 schema 与 OperationHostPort。组合根提供可信状态和行为接口，操作组件不直接调用模型或系统。非秘密草稿由应用级 OperationDrafts 保留到本次应用退出；秘密离开卡片即撤销。模板不可用时以同一操作投影显示默认审批，不转换旧协议或增加权限。生产 AI 会话通过 `execution_operation_prepare` 的 `policy.parameters.v1` 场景准备参数卡片；编辑不执行，保存通过认证 IPC 提交原请求当前参数。明确执行的策略工具仍按可信风险等级直接执行或等待确认。准备、确认、取消和恢复均使用 Rust 执行服务的同一请求记录；卡片与详情共用查询，结果未知时只能查询原任务。IPC 和 journal 使用当前版本，不转换旧协议或数据库。当前系统值未经读取时明确缺失；秘密参数无可信输入接缝时禁止提交。接线入口见桌面 `assistant/operation-ui/OperationCard.tsx`；样本界面、真实 Host/客户端接缝、provider 与 OS 执行的证据分别记录。
