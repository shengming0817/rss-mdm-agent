# Codex项目协作入口

本仓参考rss-mdm的组织方式：[AGENTS.md](../../AGENTS.md)持有协作入口，[稳定规则](../rules/README.md)按职责维护。
Codex按项目目录加载AGENTS.md；修改指令后在新会话中核对实际加载来源。[官方说明](https://learn.chatgpt.com/docs/agent-configuration/agents-md)

## 仓库内容与本地入口

- AGENTS.md、稳定规则和本说明随Git交付，不固定模型账号、token、MCP凭据、个人绝对路径或机器信任设置。
- 参考仓没有入库的.codex/config.toml或hooks配置；本仓不新增全权限/自动批准配置。
- 此工作环境的fix/pr-review共享入口位于本地`.codex/skills/`，正文引用已有RSS技能，按当前目标仓执行；绝对路径入口通过Git本地exclude排除。
- 本地技能结构及正文与rss-mdm保持一致，仅将目标仓库名替换为rss-mdm-agent；不额外增加发现目录、链接或配置。
- 本地入口不随clone分发。另一个环境应在已有共享技能可取得后按实际位置配置；不能提交本机路径或假设父仓存在才能构建产品。

## 技能使用边界

共享规则/模板的相对引用从共享技能来源仓解析；源码、Git、PR、验证始终绑定rss-mdm-agent目标checkout/worktree。
产品边界以本仓PRD和规则为准，不用RSS库仓的范围或CI覆盖客户端责任。
共享Azure forge helper操作本仓PR时明确选择`ADO_REPO=rss-mdm-agent`，避免向RSS主仓或来源项目创建PR/评论。
本地fix入口由主agent完成；pr-review入口沿用参考仓的reviewer模型约定，并显式设置允许的fork_turns。
项目Codex协作设置不等于客户端产品的AI权限政策；不将工具自身的执行批准配置复制到产品引擎。
