# 执行核心来源

当前准入直接消费产品 permit，见[执行应用服务](../../crates/execution-app/README.md)。本页保留在用核心的提取来源。

对应#2397、#2399、#2400；查阅日期2026-09-13 UTC。能力为按本产品PRD重新设计的Rust实现，不复制prmonitor执行/批准方式，不修改来源仓。原产品来源revision继续以[来源索引](sources.md)为准。

## 一手Rust源码对标

- ref: tokio tokio/src/sync/oneshot.rs@75fef53d0a8590c2d1dbb63672aa7b7d1ef51155 — [一次发送与关闭竞争](https://github.com/tokio-rs/tokio/blob/75fef53d0a8590c2d1dbb63672aa7b7d1ef51155/tokio/src/sync/oneshot.rs#L1516-L1548)。采纳单一赢家；交互须可恢复并区分回答/取消/过期，故使用显式状态和条件提交输出，不依赖channel或drop。
- ref: cargo crates/cargo-platform/src/lib.rs@7941be6fb416b4cd9666aef7b858dfea25587a8c — [要求与外部事实分离](https://github.com/rust-lang/cargo/blob/7941be6fb416b4cd9666aef7b858dfea25587a8c/crates/cargo-platform/src/lib.rs#L24-L42)。采纳纯匹配；产品必须保留四种状态，不能压缩为bool。
- ref: wasmtime crates/wasi/src/ctx.rs@817c58787f432bcdbbb87679011f72c5bc80dbda — [默认能力边界](https://github.com/bytecodealliance/wasmtime/blob/817c58787f432bcdbbb87679011f72c5bc80dbda/crates/wasi/src/ctx.rs#L47-L63)、[显式路径权限](https://github.com/bytecodealliance/wasmtime/blob/817c58787f432bcdbbb87679011f72c5bc80dbda/crates/wasi/src/ctx.rs#L253-L272)。借鉴逐维显式约束和默认拒绝；C06只匹配快照，不创建WASI或原生沙箱。

以上源码已在探索阶段按固定revision读取，仅借鉴语义，不复制源码/数据/版权正文；目标实现按本仓MIT交付。实际基础依赖由Cargo.lock持有版本与校验和。C01规范编码仍使用既有owner，未增加第二套摘要算法。

## 字段演进、幂等与诊断参考

- ref: Rust Reference [struct patterns](https://doc.rust-lang.org/stable/reference/patterns.html#struct-patterns)：生产入口穷尽解构，字段新增触发编译失败；由既有类型与编译边界持有契约。
- ref: Temporal [mutable_state_impl.go](https://github.com/temporalio/temporal/blob/main/service/history/workflow/mutable_state_impl.go)：借鉴持久结果与命令身份绑定的幂等语义；本交互只保存有界命令与终态，不引入工作流服务或新摘要实现。
- ref: Kubernetes apimachinery [errors.go](https://github.com/kubernetes/apimachinery/blob/master/pkg/api/errors/errors.go)：借鉴稳定Reason和结构化原因；公开错误保留闭合 code，不转存异常字符串。

## C09 生命周期

对应 #2402，查阅日期 2026-09-16 UTC。按本产品需求重写，不复制上游源码或签发/执行平台：

- ref: tokio tokio/src/process/mod.rs@75fef53d0a8590c2d1dbb63672aa7b7d1ef51155 — [请求终止与等待退出](https://github.com/tokio-rs/tokio/blob/75fef53d0a8590c2d1dbb63672aa7b7d1ef51155/tokio/src/process/mod.rs#L1240-L1251)。采纳取消、退出与结果分离；C09 额外要求整次受控活动停止及独立目标核实，不用进程退出证明回滚或安装成功。

#2434 入口拆分对标（2026-09-19 UTC）：ref: raft-rs src/raw_node.rs@10c6e9db6792b85c81784e44fc278f895d5f0ab0 — [本地提案与消息入口汇入共同状态机](https://github.com/tikv/raft-rs/blob/10c6e9db6792b85c81784e44fc278f895d5f0ab0/src/raw_node.rs#L346-L410)。借鉴分立入口共享状态机制；本实现进一步以 CommandEvent/ObservationEvent 区分输入，验证器只属于观察路径，不复制 Raft 协议或引入依赖。

C09 使用一个有界当前快照及派生建议，不建立通用工作流引擎。首次派发所有权和 intent 的 SQLite 原子性由 C18 持有。


## C10/C11 脚本与软件计划

对应 #2403/#2404，查阅日期 2026-09-17 UTC。以下固定 revision 的源码已直接读取，仅借鉴机制，自有 Rust 实现按本仓 MIT 交付，没有复制源码或加入上游运行时依赖。

- ref: Rust library/std/src/process.rs@f8297e351a40c1439a467bbbb6879088047f50b3 — [独立参数、环境与进程描述](https://github.com/rust-lang/rust/blob/f8297e351a40c1439a467bbbb6879088047f50b3/library/std/src/process.rs#L594-L718)。采纳 argv 原子值和显式环境；本次仅生成 C01 描述，不调用 Command、不证明 sandbox。
- ref: PowerShell src/Microsoft.PowerShell.ConsoleHost/host/msh/CommandLineParameterParser.cs@411d5fee10110d9881a909804f9d4eb1a06052ea — [`-File` 参数解析](https://github.com/PowerShell/PowerShell/blob/411d5fee10110d9881a909804f9d4eb1a06052ea/src/Microsoft.PowerShell.ConsoleHost/host/msh/CommandLineParameterParser.cs#L1367-L1418)。原参数绑定编译曾采用 `-Name:` 后独立值项及显式布尔编码；#2618 退役该未接入生产的绑定入口，当前生产消费已展开的字面参数，不再转换参数类型。文件启动仍使用固定非交互约定，不支持 Windows PowerShell 5.1。
- ref: Bash shell.c、variables.c@b8c60bc9ca365f8261fa97900b6fa939f6ebc303 — [非交互启动文件](https://git.savannah.gnu.org/cgit/bash.git/tree/shell.c?id=b8c60bc9ca365f8261fa97900b6fa939f6ebc303#n1210)。使用固定 argv 和清空继承环境，拒绝 BASH_ENV/ENV/SHELLOPTS/BASHOPTS；不承诺静态证明任意脚本安全。
- ref: nix-installer src/plan.rs@36ed46823d0e0dc40b11141ee921789664e44a1c — [计划数据与执行分离](https://github.com/DeterminateSystems/nix-installer/blob/36ed46823d0e0dc40b11141ee921789664e44a1c/src/plan.rs#L14-L28)。采纳纯描述结果，C11 只给一个下一步，不引入其 action 队列/持久化状态机。
- ref: winget-cli Versions.cpp、PackageVersionSelection.cpp@5b62860167520b1503b3880d5a026809eb07c6f4 — [生态版本规则](https://github.com/microsoft/winget-cli/blob/5b62860167520b1503b3880d5a026809eb07c6f4/src/AppInstallerSharedLib/Versions.cpp#L109-L159)、[版本选择](https://github.com/microsoft/winget-cli/blob/5b62860167520b1503b3880d5a026809eb07c6f4/src/AppInstallerRepositoryCore/PackageVersionSelection.cpp#L13-L117)。版本保留不透明原文，由生态比较器提供绑定操作数的结果；不复制 WinGet 平台实现或用通用 SemVer 代替生态语义。

C01 在 V1 内原地扩充唯一启动结构，C06 与 C09 fixtures 同步迁移；不保留旧 reader/双结构，不增加新协议版本。C10 最初直接冻结 C01；#2618 将它收窄为真实生产消费者共同使用的纯调用编译，输出现有 LaunchSpec，完整冻结留在产品外层。C11 不接入执行摘要，安装核实决策与 C09 执行生命周期分别保持单一职责。

PR #1029 安全审查修正：参考 [.NET startup hook](https://github.com/dotnet/runtime/blob/main/docs/design/features/host-startup-hook.md) 和 [dyld 环境配置](https://github.com/apple-oss-distributions/dyld/blob/main/dyld/DyldProcessConfig.cpp)，普通环境映射禁止 loader/runtime 启动控制命名空间。此限制防止通过模板或参数环境预加载计划外代码；仍不替代实际 OS sandbox 或解释器依赖验证。
