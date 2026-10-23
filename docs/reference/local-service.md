# 本机服务与平台原语来源

本项为独立重写，参考原生 OS 接缝及 Rust 调用形态，不复制服务端身份或凭据平台。

- ref: tokio tokio/src/net/windows/named_pipe.rs@75fef53d0a8590c2d1dbb63672aa7b7d1ef51155。来自锁定 tokio 1.53.1 的 Cargo 源码及 .cargo_vcs_info，核对显式 SECURITY_ATTRIBUTES、拒绝远程 peer 和 first pipe instance。
- ref: windows-rs crates/libs/sys/src/Windows/Win32/System/JobObjects/mod.rs@32c3144490c016fe496a0aed769bce60987a2e9d。锁定 windows-sys 0.61.2 的原生声明；进程组语义对照 [Microsoft Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)。
- ref: Apple Foundation/NSXPCConnection.h@macOS-SDK-26.4。核对公开 effectiveUserIdentifier、auditSessionIdentifier、processIdentifier 和 macOS 13+ setCodeSigningRequirement；不调用私有 audit-token API。[公开 peer requirement](https://developer.apple.com/documentation/foundation/nsxpcconnection/setcodesigningrequirement(_:))。
- [Node 24.14.1 官方校验表](https://nodejs.org/download/release/v24.14.1/SHASUMS256.txt)：新增 win-x64 zip SHA-256 为 6e50ce5498c0cebc20fd39ab3ff5df836ed2f8a31aa093cecad8497cff126d70。候选构建仍须实际核对版本和 SQLite ABI，不因登记 archive 就宣称 Windows 验证通过。

平台、威胁与未覆盖场景见[实验室指南](../guides/local-service-lab.md)。实现源码、跨编译结果和真实平台验收分别记载。

Windows RSS 登录采用自有 Credential Provider V2 与独立 `device-access` 离线 SQLite owner，Agent 与 DLL 静态消费同一实现。SYSTEM DPAPI 保护 TOTP/恢复材料；登录不依赖服务 IPC、网络或后台会话。密码验证仍由 LSA 完成，组件不保存 Windows 密码。组件运维及验证边界见[实验室指南](../guides/local-service-lab.md#windows-rss-登录组件x64)。

- ref: [Microsoft Windows-classic-samples CredentialProvider](https://github.com/microsoft/Windows-classic-samples/tree/434f6002bdf9cf9829406c3ff2b33387982d6168/Samples/CredentialProvider/cpp)：参考 `CSampleCredential.cpp`、`helpers.cpp` 的 COM V2 与 Kerberos 序列化，独立 Rust 实现；不使用网络样例或 Provider Filter。
- ref: [RFC 6238 §4–5](https://www.rfc-editor.org/rfc/rfc6238#section-4)：使用 ring HMAC-SHA1、最高匹配时间步与持久重放水位。恢复密钥独立随机 256 位；重复使用与现有凭据托管/轮换协议由产品持有。
