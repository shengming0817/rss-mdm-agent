import { reactive } from "vue";
import type { BackendTask, SelfServicePort, Snapshot } from "./types";

export function createController(
  port: SelfServicePort | null,
  preview: Snapshot,
) {
  const state = reactive({
    snapshot: port ? (null as Snapshot | null) : preview,
    page: "home",
    item: null as BackendTask | null,
    taskId: "",
    loading: false,
    busy: false,
    uncertain: false,
    error: "",
    after: null as string | null,
    previous: [] as (string | null)[],
  });
  let epoch = 0;
  let active = true;
  async function refresh() {
    if (!port || !active || state.loading) return;
    const call = ++epoch;
    state.loading = true;
    try {
      const value = await port.snapshot({
        after: state.after,
        selected: state.taskId || null,
      });
      if (!active || call !== epoch) return;
      state.snapshot = value;
      if (
        state.item &&
        !value.available.some(
          (t) =>
            t.task === state.item?.task &&
            t.attempt === state.item?.attempt &&
            t.revision === state.item?.revision,
        )
      )
        state.item = null;
      if (
        state.taskId &&
        (value.selected ||
          value.requests.some((t) => t.action.requestId === state.taskId))
      )
        state.uncertain = false;
      if (!state.uncertain) state.error = "";
    } catch {
      if (active && call === epoch)
        state.error = "无法读取执行服务；原任务保留，请稍后刷新";
    } finally {
      if (active && call === epoch) state.loading = false;
    }
  }
  function select(task: BackendTask) {
    if (state.busy || state.uncertain || !task.userInitiated) return;
    state.item = { ...task };
    state.taskId = task.request;
  }
  async function confirm() {
    await submit(state.item, false);
  }
  async function confirmPreparation(item: BackendTask) {
    await submit(item, true);
  }
  async function submit(item: BackendTask | null, confirmation: boolean) {
    if (!port || !item || state.busy || state.uncertain) return;
    state.taskId = item.request;
    state.busy = true;
    state.error = "";
    try {
      const result = await (confirmation ? port.confirm : port.execute)({
        request: item.request,
        task: item.task,
        attempt: item.attempt,
        revision: item.revision,
      });
      if (!active) return;
      state.taskId = result.request;
      if (result.confirmationRequired) state.error = "此操作仍等待本人确认";
      else state.item = null;
    } catch {
      if (!active) return;
      state.uncertain = true;
      state.error = "提交结果未确认；查询原任务，不能创建新尝试";
    } finally {
      if (active) {
        state.busy = false;
        await refresh();
      }
    }
  }
  async function cancel(requestId: string) {
    if (!port || state.busy) return;
    state.busy = true;
    try {
      await port.cancel({ requestId });
    } catch {
      if (active) state.error = "取消请求未确认；请查询原任务状态";
    } finally {
      if (active) {
        state.busy = false;
        await refresh();
      }
    }
  }
  function navigate(page: string) {
    state.page = page;
  }
  async function next() {
    if (!state.snapshot?.next || state.loading) return;
    state.previous.push(state.after);
    state.after = state.snapshot.next;
    await refresh();
  }
  async function previous() {
    if (!state.previous.length || state.loading) return;
    state.after = state.previous.pop() ?? null;
    await refresh();
  }
  async function first() {
    if (state.loading) return;
    state.after = null;
    state.previous = [];
    await refresh();
  }
  function dispose() {
    active = false;
    ++epoch;
  }
  return {
    state,
    interactive: !!port,
    refresh,
    select,
    confirm,
    confirmPreparation,
    cancel,
    navigate,
    next,
    previous,
    first,
    dispose,
  };
}
export type Controller = ReturnType<typeof createController>;
