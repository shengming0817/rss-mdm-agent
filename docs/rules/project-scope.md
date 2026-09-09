# 项目范围

产品需求、阶段与排除项以[PRD](../product/rss-mdm-agent-prd.md)为准。
本仓拥有客户端UI、AI adapter、本地执行/交互模型、journal和后续平台执行器；当前范围不因协作工具配置而扩大。

- rss-mdm拥有企业设备/注册主体、Group/Scope/Policy/Resource、软件源发布、产品授权和Agent wire producer。
- RSS只提供已接纳公共机制。新增产品crate不自动进入RSS，不复制通用消息/命令引擎。
- prmonitor是AI/UI提取来源，不是运行依赖；不迁入PR业务、远程终端或消息集成。
- S1仅控制链与显式测试执行器。真实OS脚本/安装、特权服务、企业接线和T3属于后续交付。
- Linux为客户端宿主设计维度，不据此宣称Linux已是rss-mdm支持的受管平台。

区分需求目标、来源已有能力、当前代码与真实验证；设备、用户、tenant和授权边界不得隐含省略。
