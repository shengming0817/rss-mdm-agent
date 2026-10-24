import { reactive, type InjectionKey } from "vue";
import type { RecentTask, SelfServicePort } from "./types";
export function createActivity(
  port: Pick<SelfServicePort, "recent_tasks"> | null,
) {
  const state = reactive({
    items: [] as RecentTask[],
    next: null as string | null,
    loaded: false,
    loading: false,
    error: "",
  });
  let active = true,
    epoch = 0,
    pages = 0;
  let seen = new Set<string>();
  async function load(append = false) {
    if (!port || !active || state.loading || (append && !state.next)) return;
    const call = epoch;
    let before = append ? state.next : null;
    state.loading = true;
    try {
      // ref: TanStack/query packages/query-core/src/infiniteQueryBehavior.ts
      // Refresh the browsed page count using fresh cursors, then publish atomically.
      const visited = new Set(append ? seen : []);
      const items = new Map(
        (append ? state.items : []).map((item) => [
          item.details.action.requestId,
          item,
        ]),
      );
      let count = 0;
      do {
        const page = await port.recent_tasks({ before });
        if (!active || call !== epoch) return;
        if (
          page.next &&
          (page.next === before ||
            visited.has(page.next) ||
            visited.size >= 2048)
        )
          throw Error("cursor");
        if (before) visited.add(before);
        for (const item of page.items)
          if (!items.has(item.details.action.requestId))
            items.set(item.details.action.requestId, item);
        before = page.next;
        count++;
      } while (!append && before && count < pages && items.size < 512);
      state.items = [...items.values()].slice(0, 512);
      state.next = state.items.length >= 512 ? null : before;
      pages = append ? pages + count : count;
      seen = visited;
      state.loaded = true;
      state.error = "";
    } catch {
      if (active && call === epoch)
        state.error = "动态读取未完成，已有记录待核实；请重试原请求。";
    } finally {
      if (active && call === epoch) state.loading = false;
    }
  }
  function dispose() {
    active = false;
    epoch++;
    state.items = [];
    seen.clear();
  }
  return { state, available: !!port, load, dispose };
}
export type ActivityController = ReturnType<typeof createActivity>;
export const activityKey: InjectionKey<ActivityController> =
  Symbol("task-activity");
export const taskPhase: Record<string, string> = {
  waiting: "等待执行",
  confirmationRequired: "等待本人确认",
  accepted: "已接收，启动待核实",
  running: "执行中",
  outcomeUnknown: "效果未知",
  executionEnded: "进程已结束，效果待核实",
  verified: "结果已核实",
  cancelled: "已确认取消",
  admissionDenied: "准入被拒绝",
  failedBeforeDispatch: "启动前失败",
  approvalRequired: "等待授权",
};
