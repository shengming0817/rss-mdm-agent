# ACP–A2UI 开发

接入由 [ai-access](../../packages/ai-access/README.md)、[ai-client](../../packages/ai-client/README.md)和 [ai-ui-bridge](../../packages/ai-ui-bridge/README.md) 提供。产品 wire 由唯一 schema 生成。

1. 可信组合根建立 access service，绑定认证后的 caller 与官方 SDK stream，不能从请求正文获得身份。
2. 客户端协商、创建和恢复会话；普通 ACP prompt 等待终态，产品扩展另行表达接纳与附着。
3. Vue 挂载 RuntimeSurface；renderer 只保存显示缓存，客户端投影负责稳定恢复内容。
4. 收到 resync_required 后显式 restore，先收齐同一水位历史再替换投影。分页 token 只定位读视图，不授予身份。

标准权限使用实时回调；恢复历史不能重建原生回调。断开、卸载和取消分别操作，不自动取消执行。期限在提交时再次检查，竞争失败方不能重试，首答方丢回执仍使用原命令。

```sh
pnpm test:ai-access
pnpm check:assistant
```

浏览器配置与产品 fixture 见[桌面指南](desktop-development.md)。内存 Host/Store 仅验证接入语义，不证明持久回执、真实 provider 或 OS 隔离。官方 processor 异常先使卡片失效，再显式恢复；逐条内存更新不等于事务。

调用方必须传播 owner cancellation 并有界等待；关闭超时表示仍有未结算资源，不能据此宣称 provider 终止。公开错误使用闭合分类，不回显底层 payload 或 cause。协议与 renderer 来源见[AI Runtime 来源](../reference/ai-runtime.md)。
