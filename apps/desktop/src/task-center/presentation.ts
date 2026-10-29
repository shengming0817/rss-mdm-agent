import type { Snapshot } from "valtio/vanilla";
import type {
  TaskDetail,
  TaskNode,
  TaskRef,
  TaskSource,
} from "../self-service/types";
import { taskPresentation } from "../assistant/execution-presentation";
import { taskPhase } from "./controller";
export const sourceLabels: Record<TaskSource, string> = {
  human: "人工",
  ai: "AI",
  policy: "后台策略",
};
export const actionLabels = {
  cancel: "请求取消",
  retry: "重试已知失败的任务",
  pause: "暂停本任务下载",
  resume: "继续本任务下载",
};
export function referenceLabel(reference: Snapshot<TaskRef>): string {
  return reference.kind === "request"
    ? reference.request
    : reference.kind === "sampling"
      ? `${reference.query.id} · ${reference.query.revision}`
      : reference.id;
}
export const recordedTime = (value: number | null) =>
  value == null ? "未提供记录时间" : new Date(value).toLocaleString();
const explanations: Record<string, string> = {
  waiting: "等待原执行条件与准入；页面不会自动重试。",
  confirmationRequired:
    "核对可信来源、具体动作与期限；本人确认仍不能覆盖企业或 OS 权限。",
  admissionDenied: "原执行服务拒绝本次准入。页面和 AI 确认不能覆盖该决定。",
  accepted: "原执行意图已记录，派发与设备效果尚未确认。",
  running: "原执行器已接收任务；完成与效果需要分别核实。",
  outcomeUnknown:
    "尚未取得完整回执或实际状态。查询同一原请求，不要盲目重新执行。",
  executionEnded:
    "进程或操作已返回，等待独立读取实际状态；退出码不代表核实成功。",
  verified:
    "以原执行服务记录的独立核实结论为准；核实完成仍可能未达预期或无效果。",
  cancelled: "原服务已记录可信取消且无效果事实；不承诺回滚任何其它外部修改。",
  failedBeforeDispatch: "可信证据确认派发前失败；后续动作仍由服务重新核对。",
};
export function presentTask(task: Snapshot<TaskDetail>) {
  if (task.facts.kind === "execution") {
    const status = task.facts.value.status,
      view = taskPresentation(task.facts.value);
    const success =
      status.phase === "verified" && status.assessment === "satisfied";
    const attention =
      view.attention ||
      ["executionEnded", "outcomeUnknown"].includes(status.phase) ||
      (status.phase === "verified" && !success);
    return {
      label: view.label,
      description: `${explanations[status.phase]}${status.software ? ` ${view.stage}` : ""}${view.cancel ? ` ${view.cancel}` : ""}`,
      success,
      attention,
      color: success
        ? "green"
        : ["admissionDenied", "failedBeforeDispatch"].includes(status.phase) ||
            status.assessment === "notSatisfied"
          ? "red"
          : attention
            ? "gold"
            : "default",
    };
  }
  const label = taskPhase[task.stage] ?? "原来源状态待核对";
  return {
    label,
    success: false,
    attention: ["failed", "unknown", "awaitingConfirmation"].includes(
      task.stage,
    ),
    color:
      task.stage === "failed"
        ? "red"
        : ["unknown", "awaitingConfirmation"].includes(task.stage)
          ? "gold"
          : "default",
    description:
      task.facts.kind === "management"
        ? "管理来源记录、设备执行和实际效果分别核对。"
        : task.facts.kind === "sampling"
          ? "采集完成、枚举完整和上传回执分别记录。"
          : "准备记录保留原请求；受理与实际执行结果分别核对。",
  };
}
const nodeLabels: Record<string, string> = {
  download: "下载内容",
  prepare: "准备内容",
  install: "安装",
  update: "更新",
  uninstall: "卸载",
  detect: "读取软件事实",
  cleanup: "清理本任务临时内容",
  begin: "开始",
  end: "结束",
  pending: "等待结果",
  completed: "已完成",
  failed: "失败",
  running: "执行中",
  succeeded: "执行已结束",
};
export function nodeLabel(node: Snapshot<TaskNode>) {
  return (
    taskPhase[node.stage] ??
    nodeLabels[node.stage] ??
    node.stage.replace(/[_:]/g, " · ")
  );
}
export function detectionLabel(
  state: Snapshot<NonNullable<TaskDetail["softwareState"]>>,
) {
  if (state.kind === "present") return `已检测安装 · ${state.version}`;
  return state.kind === "absent" ? "已检测未安装" : "软件事实尚未确定";
}

const activityLabels: Record<string, string> = {
  queued: "已排队",
  offered: "已提供执行任务",
  dispatched: "已派发",
  received: "来源已接收",
  unknown: "未确认",
  not_started: "尚未启动",
  running: "执行中",
  succeeded: "来源报告执行成功；效果另行核实",
  failed: "来源已记录失败",
  waiting_reboot: "等待重启后核实",
  published: "已发布",
  applied: "来源报告已应用",
  rejected: "已拒绝",
  timed_out: "已超时",
  superseded: "已被后续版本替代",
  cancelled: "来源已记录取消",
  verified: "来源已核实",
  diverged: "与预期不一致",
  waiting: "等待核实",
  unverifiable: "当前无法核实",
  partial: "仅部分报告",
  acknowledged: "来源已接收报告",
  pending: "等待决定",
  accepted: "已受理",
  no_change: "未要求变化",
  expired: "已过期",
  cancellation_requested: "已请求取消，终止待核实",
  assigned: "已分配",
  unavailable: "当前不可用",
};
export const activityLabel = (value: string | null) =>
  value == null ? "未取得或不适用" : (activityLabels[value] ?? value);
