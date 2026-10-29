import type {
  DiagnosticPort,
  DiagnosticPreview,
  DiagnosticScopes,
} from "./native";
export function createDiagnostics(port?: DiagnosticPort) {
  const state = {
    scopes: {
      host: true,
      service: true,
      device: true,
      execution: true,
    } as DiagnosticScopes,
    preview: undefined as DiagnosticPreview | undefined,
    busy: false,
    message: "",
  };
  let epoch = 0,
    disposed = false;
  function clear() {
    ++epoch;
    const snapshot = state.preview?.snapshot;
    state.preview = undefined;
    state.busy = false;
    state.message = "";
    if (snapshot) void port?.discard(snapshot).catch(() => {});
  }
  async function prepare() {
    if (!port || state.busy || disposed) return;
    clear();
    state.busy = true;
    const call = epoch;
    try {
      const preview = await port.prepare({ ...state.scopes });
      if (disposed || call !== epoch) {
        void port.discard(preview.snapshot).catch(() => {});
        return;
      }
      state.preview = preview;
    } catch {
      if (!disposed && call === epoch)
        state.message = "无法生成诊断预览，请重试。";
    } finally {
      if (call === epoch) state.busy = false;
    }
  }
  async function save() {
    const preview = state.preview;
    if (!port || !preview || state.busy || disposed) return;
    const call = epoch;
    state.busy = true;
    state.message = "";
    try {
      const saved = await port.export(preview.snapshot);
      if (disposed || call !== epoch) return;
      state.message = saved ? "脱敏诊断已保存。" : "已取消导出。";
      if (saved) state.preview = undefined;
    } catch {
      if (!disposed && call === epoch)
        state.message = "诊断未导出，请刷新预览或检查保存位置。";
    } finally {
      if (call === epoch) state.busy = false;
    }
  }
  return {
    state,
    available: !!port,
    prepare,
    save,
    clear,
    dispose() {
      clear();
      disposed = true;
    },
  };
}
