import type {
  ExecutionTaskDetails,
  SoftwareDiagnostic,
} from "@rss-mdm-agent/execution-bindings/task-details";
type TaskPhase = ExecutionTaskDetails["status"]["phase"];
export function executionPhase(value: TaskPhase): string {
  switch (value) {
    case "waiting":
      return "等待执行条件";
    case "admissionDenied":
      return "执行准入被拒绝";
    case "confirmationRequired":
      return "等待用户确认本次动作";
    case "approvalRequired":
      return "执行服务记录：需要管理员批准";
    case "accepted":
      return "执行意图已记录，尚未确认派发";
    case "running":
      return "执行器已接收，设备效果尚未确认";
    case "outcomeUnknown":
      return "设备效果未知，需要可信核对";
    case "executionEnded":
      return "执行已结束，等待效果验证";
    case "verified":
      return "执行结果已核实";
    case "failedBeforeDispatch":
      return "可信证据确认派发前失败";
    case "cancelled":
      return "可信证据确认取消且无效果";
  }
}
export function softwareStatus(value: SoftwareDiagnostic): string {
  switch (value) {
    case "cleanupPending":
      return "临时安装文件尚待安全清理，资源占用保留";
    case "cleanupUnverified":
      return "无法确认临时目录归属，需要人工核实";
    case "awaitingDetection":
      return "等待独立软件检测";
    case "restartPending":
      return "安装器要求重启设备；重启后重新核实";
    case "detectionUnavailable":
      return "软件检测不可用，请核对设备与读取权限";
    case "unrecognizedVersion":
      return "检测到未知软件内容，需要人工核实";
    case "detectionBudgetExceeded":
      return "软件检测预算耗尽，等待下一次有界核实";
    case "desiredStateObserved":
      return "已观察到目标软件状态；不代表后台活动已终止";
    case "desiredStateMissing":
      return "已检测软件状态，尚未达到目标";
  }
  const exhaustive: never = value;
  return exhaustive;
}

export function taskPresentation(details: ExecutionTaskDetails) {
  const s = details.status;
  const label =
    s.phase === "verified"
      ? s.assessment === "satisfied"
        ? "已核实符合预期"
        : s.assessment === "notSatisfied"
          ? "已核实未达预期"
          : s.assessment === "noEffect"
            ? "已核实无效果"
            : "效果仍需核对"
      : executionPhase(s.phase);
  return {
    label,
    stage: s.software ? softwareStatus(s.software) : executionPhase(s.phase),
    attention:
      [
        "confirmationRequired",
        "approvalRequired",
        "outcomeUnknown",
        "admissionDenied",
        "failedBeforeDispatch",
      ].includes(s.phase) || s.assessment === "notSatisfied",
    cancel: s.cancelRequested ? "已请求取消，终止及效果仍由执行服务核对。" : "",
  };
}
