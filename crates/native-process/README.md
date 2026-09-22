# native-process

产品私有进程范围及其运行文件原语。固定 launcher 校验 Node/bootstrap manifest，提供 macOS 进程组、Windows Job Object 的当前 owner 回收；持久 scope 只用于只读确认不存在。私有文件模块提供 Unix 权限、Windows ACL/DPAPI 与原生输入，不拥有服务身份或业务授权。

Rust Ready/Scope 是跨语言进程记录唯一声明源，node scripts/generate-process-contract.mjs --check 检查 TS 投影。运行目录 manifest 固定入口，不支持任意命令。此包不发布为 RSS 通用库。平台证据见[实验室指南](../../docs/guides/local-service-lab.md)。
