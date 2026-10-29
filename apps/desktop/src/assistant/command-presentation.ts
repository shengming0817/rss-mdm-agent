import type { CommandView } from "@rss-mdm-agent/ai-client";
export function failureNote(command: CommandView) {
  switch (command.failure?.retry) {
    case "same_command":
      return "仅允许核对同一原命令；保留原 ID、内容和期限，不生成替代请求。";
    case "reconcile_first":
      return "先读取原会话与原命令，核实接纳和执行状态，再按原服务结果处理。";
    case "never":
      return "原服务不允许重试此命令；读取原记录，不重新发送。";
    default:
      return "";
  }
}
export function cancelNote(command: CommandView) {
  if (command.acknowledgement?.type === "queued_cancelled")
    return "已取消排队，未发送给模型。";
  switch (
    command.acknowledgement?.type === "cancel"
      ? command.acknowledgement.confirmation
      : command.cancelDispatched
  ) {
    case "request_only":
      return command.state === "terminal"
        ? "取消请求已发出；模型终止事实已记录。"
        : "取消已发送给模型；等待模型终止事实。";
    case "already_terminal":
      return "取消请求返回模型已终止；设备效果仍需独立核对。";
    case "unsupported":
      return "当前模型不支持取消；原运行状态保持不变。";
    default:
      return "";
  }
}
