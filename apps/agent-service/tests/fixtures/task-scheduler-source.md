# Task Scheduler 契约来源

`task-scheduler.xsd` 是 Microsoft Learn [Task Scheduler Schema](https://learn.microsoft.com/en-us/windows/win32/taskschd/task-scheduler-schema) 公布的完整 XSD，2026-10-18 获取，仅去除行尾空格并统一末尾换行，未修改 schema 内容。

网页原始代码块 SHA-256：`158a01e2958e96fa8b5539ab73e8937c3cf9440ce3ebf4f6b0aa366bae1b0035`。

此官方 schema 的 Task version 固定为 1.3。生产退管模板直接生成 1.3，测试不改写版本或裁剪 schema；macOS 用系统 xmllint 验证实际生产输出及拒绝反例。该主机契约验证不替代 Windows 原生任务注册与断电验收。

上游代码示例按 [MicrosoftDocs/win32 LICENSE-CODE](https://github.com/MicrosoftDocs/win32/blob/docs/LICENSE-CODE) 发布；许可原文保留于同目录 `task-scheduler-LICENSE`。

入库 XSD SHA-256：`972300ebaf6a3de884882d7189d86a71925c2a6892c230e4285e2e147d670e40`。
