# 本机服务与平台原语来源

本项为独立重写，参考原生 OS 接缝及 Rust 调用形态，不复制服务端身份或凭据平台。

- ref: tokio tokio/src/net/windows/named_pipe.rs@75fef53d0a8590c2d1dbb63672aa7b7d1ef51155。来自锁定 tokio 1.53.1 的 Cargo 源码及 .cargo_vcs_info，核对显式 SECURITY_ATTRIBUTES、拒绝远程 peer 和 first pipe instance。
- ref: windows-rs crates/libs/sys/src/Windows/Win32/System/JobObjects/mod.rs@32c3144490c016fe496a0aed769bce60987a2e9d。锁定 windows-sys 0.61.2 的原生声明；进程组语义对照 [Microsoft Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)。
- ref: Apple Foundation/NSXPCConnection.h@macOS-SDK-26.4。核对公开 effectiveUserIdentifier、auditSessionIdentifier、processIdentifier 和 macOS 13+ setCodeSigningRequirement；不调用私有 audit-token API。[公开 peer requirement](https://developer.apple.com/documentation/foundation/nsxpcconnection/setcodesigningrequirement(_:))。
- [Node 24.14.1 官方校验表](https://nodejs.org/download/release/v24.14.1/SHASUMS256.txt)：新增 win-x64 zip SHA-256 为 6e50ce5498c0cebc20fd39ab3ff5df836ed2f8a31aa093cecad8497cff126d70。候选构建仍须实际核对版本和 SQLite ABI，不因登记 archive 就宣称 Windows 验证通过。

平台、威胁与未覆盖场景见[实验室指南](../guides/local-service-lab.md)。实现源码、跨编译结果和真实平台验收分别记载。
