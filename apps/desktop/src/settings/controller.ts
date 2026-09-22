import { reactive } from "vue";
import type { HostStatus } from "@rss-mdm-agent/ai-contract";
import type { nativeHost } from "./native";
export function createHostSettings(port: ReturnType<typeof nativeHost>) {
  const state = reactive({
    status: undefined as HostStatus | undefined,
    busy: false,
    loading: false,
    message: "",
    readError: "",
  });
  let epoch = 0,
    disposed = false;
  async function refresh() {
    if (!port || state.busy || state.loading || disposed) return;
    const current = epoch;
    state.loading = true;
    try {
      const next = await port.read();
      if (!disposed && current === epoch) {
        state.status = next;
        state.readError = "";
      }
    } catch {
      if (!disposed && current === epoch)
        state.readError = "无法读取 AI Host 状态，请重试。";
    } finally {
      if (current === epoch) state.loading = false;
    }
  }
  async function restart() {
    if (!port || state.busy || !state.status || disposed) return false;
    epoch++;
    state.loading = false;
    state.busy = true;
    state.message = "";
    try {
      const next = await port.restart(state.status.generation);
      if (disposed) return false;
      state.status = next;
      state.readError = "";
      return next.phase === "ready";
    } catch {
      if (!disposed) state.message = "重启结果未确认，请刷新状态后再操作。";
      return false;
    } finally {
      state.busy = false;
    }
  }
  async function exportDiagnostics() {
    if (!port || state.busy || disposed) return;
    state.busy = true;
    state.message = "";
    try {
      const saved = await port.export();
      if (!disposed)
        state.message = saved ? "脱敏诊断已保存。" : "已取消导出。";
    } catch {
      if (!disposed) state.message = "诊断未导出，请检查保存位置后重试。";
    } finally {
      state.busy = false;
    }
  }
  return {
    state,
    available: !!port,
    refresh,
    restart,
    exportDiagnostics,
    dispose() {
      disposed = true;
      epoch++;
    },
  };
}
export type HostSettings = ReturnType<typeof createHostSettings>;
export const diagnosticMessage = (status?: HostStatus) => {
  switch (status?.diagnostic?.code) {
    case "runtime_missing":
      return status.source === "development_override"
        ? "缺少开发运行包。请构建运行包并设置开发路径后重启 AI Host。"
        : "安装包缺少 AI 运行资源，请重新安装完整应用。";
    case "unsupported_version":
      if (status.diagnostic.stage === "storage")
        return "当前 AI 数据库版本不受支持；旧数据已保留，请使用新的实验室数据目录。";
      return "运行包无效或版本不匹配，请使用与应用匹配的完整运行包。";
    case "runtime_invalid":
      return "运行包无效或版本不匹配，请使用与应用匹配的完整运行包。";
    case "configuration_invalid":
      return "Host 路径配置无法读取，请检查本地数据目录权限后重启 AI Host。";
    case "authentication_required":
      return "认证不可用。请检查官方工具登录或在连接设置中更新密钥，再验证连接。";
    case "storage_corrupt":
      return "本地 AI 数据无法读取，请保留数据并检查存储；不会自动清库。";
    case "cleanup_incomplete":
      return "旧进程尚未确认退出，暂不能启动新 Host。请退出应用并检查进程后重新打开。";
    case "control_closed":
      return "AI 控制连接中断，请受控重启后重新连接；请求不会自动重发。";
    case "host_exited":
      return "AI Host 已退出。可受控重启；设备任务继续按原记录核实。";
    case "readiness_timeout":
      return "AI Host 未在时限内就绪，请重启后重试。";
    case "host_start_failed":
      return "AI Host 未能启动，请检查运行包后重启。";
    default:
      return "";
  }
};
