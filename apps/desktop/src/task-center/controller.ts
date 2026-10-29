import { deepClone, proxyMap } from "valtio/vanilla/utils";
import { proxy, type Snapshot } from "valtio/vanilla";
import type {
  TaskPort,
  TaskDetail,
  TaskRef,
  TaskQuery,
  TaskControl,
  TaskAction,
  SelfServicePort,
  BackendTask,
} from "../self-service/types";

// IPC and provider objects may order their properties differently. Identity is
// the typed original value, including the complete sampling definition version.
export const taskKey = (reference: TaskRef): string =>
  JSON.stringify(
    reference.kind === "request"
      ? { kind: reference.kind, request: reference.request }
      : reference.kind === "sampling"
        ? {
            kind: reference.kind,
            query: {
              id: reference.query.id,
              revision: reference.query.revision,
            },
          }
        : { kind: reference.kind, id: reference.id },
  );
export const requestRef = (request: string): TaskRef => ({
  kind: "request",
  request,
});
export const taskTitle = (task: Snapshot<TaskDetail>): string => {
  const facts = task.facts;
  if (facts.kind === "execution")
    return `${facts.value.action.operation.action} · ${facts.value.action.operation.resource.id}`;
  if (facts.kind === "preparation") {
    const action = facts.value.action;
    return action.kind === "offered"
      ? action.offer.presentation.title
      : action.kind === "policy" &&
          typeof action.presentation.displayName === "string"
        ? action.presentation.displayName
        : "准备查询";
  }
  if (facts.kind === "sampling") return facts.value.definition.title;
  return `${facts.value.kind} · ${facts.value.phase}`;
};
export const taskPhase: Record<string, string> = {
  waiting: "等待执行",
  confirmationRequired: "等待本人确认",
  awaitingConfirmation: "等待本人确认",
  draft: "准备参数",
  ready: "待执行",
  submitting: "准备材料",
  accepted: "已接收，启动待核实",
  running: "执行中",
  outcomeUnknown: "效果未知",
  executionEnded: "进程已结束，效果待核实",
  verified: "结果已核实",
  cancelled: "取消事实已记录",
  admissionDenied: "准入被拒绝",
  failedBeforeDispatch: "启动前失败",
  failed: "准备失败",
  observed: "结果已读取",
  paused: "本任务下载已暂停",
};
const initialQuery = (): TaskQuery => ({
  view: "history",
  source: null,
  taskKind: null,
  status: null,
  from: null,
  until: null,
  subject: null,
  policy: null,
  cursor: null,
  limit: 32,
});

/** One cache of service projections; no client execution lifecycle or second activity store. */
export function createTaskCenter(
  tasks: TaskPort | null,
  port: SelfServicePort | null,
) {
  const state = proxy({
    query: initialQuery(),
    items: [] as TaskDetail[],
    selected: null as TaskDetail | null,
    next: null as string | null,
    loading: false,
    loaded: false,
    busy: false,
    uncertain: false,
    error: tasks ? "" : "本机任务服务未连接",
    taskId: "",
    byRef: proxyMap<string, TaskDetail>(),
    recent: [] as TaskRef[],
    recentNext: null as string | null,
    recentLoaded: false,
    recentAsOf: 0,
    recentLoading: false,

    remoteAvailable: false,
    scope: "",
    asOf: 0,
    lastSync: null as number | null,
    coverage: [] as { source: string; complete: boolean; reasons: string[] }[],
    retainedFrom: null as number | null,
    complete: false,
    pendingControl: null as TaskControl | null,
    node: "",
    listScroll: 0,
    listFocus: "",
    cursor: null as string | null,
    previous: [] as (string | null)[],
    page: 1,
    recentError: "",
    recentWindowed: false,
  });
  type ReadState = Snapshot<typeof state>;
  let alive = true,
    epoch = 0,
    listEpoch = 0,
    selectionEpoch = 0,
    visible = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let homeVisible = false,
    recentFlight: Promise<void> | undefined;

  let listFlight: Promise<void> | undefined;
  const detailFlights = new Map<string, Promise<TaskDetail>>();
  const readAt = new Map<string, number>();
  let listRevision = 0,
    recentBrowsing = false;
  function cache(item: TaskDetail) {
    const key = taskKey(item.reference);
    state.byRef.set(key, item);
    readAt.set(key, Date.now());
    while (state.byRef.size > 512) {
      const first = state.byRef.keys().next().value!;
      if (state.recent.some((ref) => taskKey(ref) === first))
        state.recentWindowed = true;
      state.byRef.delete(first);
      readAt.delete(first);
    }
  }

  const preparations = (current: ReadState = state) =>
    [...current.byRef.values()].flatMap((t) =>
      t.facts.kind === "preparation" ? [t.facts.value] : [],
    );
  function invalidate() {
    ++epoch;
    state.byRef.clear();
    state.recent = [];
    state.recentNext = null;
    state.recentLoaded = false;
    state.recentAsOf = 0;
    state.items = [];
    state.selected = null;
    state.next = null;
    state.pendingControl = null;
    state.node = "";
    state.uncertain = false;
    state.busy = false;
    state.loaded = false;
    detailFlights.clear();
    readAt.clear();
    recentBrowsing = false;
    state.cursor = null;
    state.previous = [];
    state.page = 1;
    state.recentError = "";
    state.recentWindowed = false;
    state.listScroll = 0;
    state.listFocus = "";
  }
  function acceptScope(scope: string) {
    if (state.scope && state.scope !== scope) {
      invalidate();
      state.scope = scope;
      state.error = "任务主体已变化，请重新读取";
      throw Error(state.error);
    }
    state.scope = scope;
  }
  async function load(append = false): Promise<void> {
    if (!tasks || !alive || (append && !state.next)) return;
    if (listFlight) return listFlight;
    const call = epoch,
      queryVersion = listEpoch;
    const query = {
      ...state.query,
      cursor: append ? state.next : state.cursor,
    };
    state.loading = true;
    listFlight = (async () => {
      try {
        const page = deepClone(await tasks.list(query));
        if (!alive || call !== epoch || queryVersion !== listEpoch) return;
        acceptScope(page.scope);
        if (page.next && page.next === query.cursor)
          throw Error("任务游标未推进");
        for (const item of page.items) {
          if (item.scope !== page.scope) throw Error("任务主体不一致");
          cache(item);
        }
        if (append) {
          state.previous.push(state.cursor);
          if (state.previous.length > 16) state.previous.shift();
          state.cursor = query.cursor;
          state.page++;
          state.listScroll = 0;
          state.listFocus = "";
        }
        state.items = page.items;
        state.next = page.next;
        listRevision++;
        state.remoteAvailable = page.remoteAvailable;
        state.asOf = page.asOf;
        state.lastSync = page.lastSync;
        state.coverage = page.coverage;
        state.retainedFrom = page.retainedFrom;
        state.complete = page.complete;
        state.loaded = true;
        if (!state.uncertain) state.error = "";
      } catch (error) {
        if (alive && call === epoch)
          state.error =
            error instanceof Error
              ? error.message
              : "无法读取任务；已有记录待核实";
      } finally {
        if (alive) {
          state.loading = false;
          listFlight = undefined;
        }
      }
    })();
    return listFlight;
  }
  async function previousPage() {
    if (!state.previous.length || state.loading) return;
    const oldCursor = state.cursor,
      oldPrevious = [...state.previous],
      oldPage = state.page,
      before = listRevision,
      call = epoch;
    state.cursor = state.previous.pop()!;
    state.page--;
    state.listScroll = 0;
    state.listFocus = "";
    await load();
    if (call === epoch && before === listRevision) {
      state.cursor = oldCursor;
      state.previous = oldPrevious;
      state.page = oldPage;
    }
  }
  const recentItems = (current: ReadState = state) =>
    current.recent
      .flatMap((ref) => {
        const item = current.byRef.get(taskKey(ref));
        return item ? [item] : [];
      })
      .sort(
        (a, b) =>
          (b.recordedAt ?? -1) - (a.recordedAt ?? -1) ||
          (taskKey(a.reference) < taskKey(b.reference)
            ? 1
            : taskKey(a.reference) === taskKey(b.reference)
              ? 0
              : -1),
      );
  async function loadRecent(append = false): Promise<void> {
    if (!tasks || !alive || (append && !state.recentNext)) return;
    if (recentFlight) return recentFlight;
    const call = epoch;
    state.recentLoading = true;
    recentFlight = (async () => {
      try {
        const cursor = append ? state.recentNext : null;
        const page = deepClone(
          await tasks.list({ ...initialQuery(), limit: 32, cursor }),
        );
        if (!alive || call !== epoch) return;
        acceptScope(page.scope);
        if (page.next && page.next === cursor) throw Error("任务游标未推进");
        const refs = new Map(state.recent.map((ref) => [taskKey(ref), ref]));
        for (const item of page.items) {
          if (item.scope !== page.scope) throw Error("任务主体不一致");
          cache(item);
          refs.set(taskKey(item.reference), item.reference);
        }
        state.recentWindowed ||= refs.size > 512;
        state.recent = [...refs.values()].slice(-512);
        if (append || !recentBrowsing) state.recentNext = page.next;
        if (append) recentBrowsing = true;
        state.recentError = "";
        state.recentLoaded = true;
        state.recentAsOf = page.asOf;
      } catch {
        if (alive && call === epoch)
          state.recentError = "最近任务读取未完成，保留事实待核实";
      } finally {
        if (alive) {
          state.recentLoading = false;
          recentFlight = undefined;
        }
      }
    })();
    return recentFlight;
  }
  async function restartRecent() {
    const call = epoch;
    await recentFlight;
    if (!alive || call !== epoch) return;
    state.recent = [];
    state.recentNext = null;
    state.recentLoaded = false;
    state.recentAsOf = 0;
    state.recentWindowed = false;
    recentBrowsing = false;
    await loadRecent();
  }
  function forget(reference: TaskRef) {
    const key = taskKey(reference);
    state.byRef.delete(key);
    readAt.delete(key);
    state.items = state.items.filter((item) => taskKey(item.reference) !== key);
    state.recent = state.recent.filter((item) => taskKey(item) !== key);
    if (state.selected && taskKey(state.selected.reference) === key) {
      state.selected = null;
      state.node = "";
    }
  }
  async function detail(reference: TaskRef, fresh = false) {
    if (!tasks || !alive) throw Error("任务服务未连接");
    const key = taskKey(reference),
      existing = detailFlights.get(key);
    if (existing) return existing;
    const cached = state.byRef.get(key);
    if (!fresh && cached && Date.now() - (readAt.get(key) ?? 0) < 2000)
      return cached;
    const call = epoch;
    const promise = tasks
      .detail(reference)
      .then((received) => {
        const value = deepClone(received);
        if (!alive || call !== epoch) throw Error("任务主体已失效");
        acceptScope(value.scope);
        cache(value);
        const index = state.items.findIndex(
          (t) => taskKey(t.reference) === key,
        );
        if (index >= 0) state.items[index] = value;
        return value;
      })
      .catch((error) => {
        if (
          alive &&
          call === epoch &&
          typeof error === "object" &&
          error !== null &&
          "code" in error &&
          error.code === "task_denied"
        )
          forget(reference);
        throw error;
      })
      .finally(() => {
        if (detailFlights.get(key) === promise) detailFlights.delete(key);
      });
    detailFlights.set(key, promise);
    return promise;
  }
  function closeTask() {
    selectionEpoch++;
    state.selected = null;
    state.node = "";
  }
  async function openTask(reference: TaskRef, node = "") {
    const call = epoch,
      selection = ++selectionEpoch;
    if (
      state.selected &&
      taskKey(state.selected.reference) !== taskKey(reference)
    ) {
      state.selected = null;
      state.node = "";
    }
    try {
      const value = await detail(reference, true);
      if (!alive || call !== epoch || selection !== selectionEpoch)
        return "stale" as const;
      if (node && !value.nodes.some((n) => n.id === node)) {
        state.selected = value;
        state.node = "";
        state.error = "原步骤已不在当前详情中，已读取原任务最新事实";
        return "unavailable" as const;
      }
      if (
        state.uncertain &&
        !state.pendingControl &&
        (value.facts.kind === "execution" ||
          (value.facts.kind === "preparation" &&
            value.facts.value.state !== "awaitingConfirmation"))
      ) {
        state.uncertain = false;
        state.error = "";
      }
      state.selected = value;
      state.taskId = reference.kind === "request" ? reference.request : "";
      state.node = node;
      return "ready" as const;
    } catch {
      if (!alive || call !== epoch || selection !== selectionEpoch)
        return "stale" as const;
      state.error = "原任务不可访问或状态未确认，请重试读取";
      return "unknown" as const;
    }
  }
  async function reconcileControl() {
    if (!tasks || !state.pendingControl) return;
    const original = { ...state.pendingControl },
      call = epoch;
    const result = await tasks.controlStatus({
      reference: original.reference,
      id: original.id,
      scope: original.scope,
    });
    if (!alive || call !== epoch) return;
    state.uncertain = result.disposition === "unknown";
    if (result.created) await openTask(result.created);
    if (!state.uncertain) {
      state.pendingControl = null;
      state.error =
        result.disposition === "confirmed"
          ? ""
          : "操作未获准或版本已变化，请重新读取";
    }
  }
  async function control(task: TaskDetail, action: TaskAction) {
    if (
      !tasks ||
      !alive ||
      state.busy ||
      state.pendingControl ||
      !task.allowedActions.includes(action)
    )
      return;
    const input: TaskControl = {
      reference: task.reference,
      id: crypto.randomUUID(),
      scope: task.scope,
      expectedRevision: task.revision,
      action,
    };
    state.pendingControl = input;
    state.busy = true;
    const call = epoch;
    try {
      const receipt = await tasks.control(input);
      if (!alive || call !== epoch) return;
      state.uncertain = receipt.disposition === "unknown";
      if (receipt.created) await openTask(receipt.created);
      if (!state.uncertain) {
        state.pendingControl = null;
        state.error =
          receipt.disposition === "confirmed"
            ? ""
            : "操作未获准或版本已变化，请重新读取";
      }
    } catch (error) {
      if (alive && call === epoch) {
        const code =
          typeof error === "object" && error !== null && "code" in error
            ? String(error.code)
            : "";
        const rejected = ["task_denied", "task_stale", "task_input"].includes(
          code,
        );
        state.uncertain = !rejected;
        if (rejected) state.pendingControl = null;
        state.error = rejected
          ? "操作未获准或版本已变化，请重新读取"
          : "控制结果未知；查询原操作，不能重复提交";
      }
    } finally {
      if (alive && call === epoch) {
        state.busy = false;
        if (action !== "retry") await openTask(task.reference, state.node);
      }
    }
  }
  async function cancel(request: string) {
    try {
      const task = await detail(requestRef(request));
      await control(task, "cancel");
    } catch {
      state.error = "无法确认原任务，未提交取消";
    }
  }
  async function confirmPreparation(item: BackendTask) {
    if (!port || state.busy || state.uncertain) return;
    state.busy = true;
    const call = epoch;
    try {
      await port.confirm({
        request: item.request,
        task: item.task,
        attempt: item.attempt,
        revision: item.revision,
      });
    } catch {
      if (alive && call === epoch) {
        state.uncertain = true;
        state.error = "确认回执未知，请查询原请求";
      }
    } finally {
      if (alive && call === epoch) {
        state.busy = false;
        await openTask(requestRef(item.request));
      }
    }
  }
  async function refresh() {
    await load();
    if (state.selected) await openTask(state.selected.reference, state.node);
    if (state.pendingControl) await reconcileControl().catch(() => {});
  }
  async function setQuery(query: Partial<TaskQuery>) {
    ++listEpoch;
    state.query = { ...initialQuery(), ...query, cursor: null };
    state.next = null;
    state.items = [];
    state.loaded = false;
    state.cursor = null;
    state.previous = [];
    state.page = 1;
    await listFlight;
    await load();
  }
  function schedule() {
    clearTimeout(timer);
    if (!alive || !visible || document.hidden) return;
    timer = setTimeout(
      async () => {
        if (homeVisible) await loadRecent();
        else await refresh();
        schedule();
      },
      [...state.byRef.values()].some((t) => t.active) || state.pendingControl
        ? 2000
        : 30000,
    );
  }
  function setVisible(value: boolean, home = false) {
    visible = value;
    homeVisible = home;
    clearTimeout(timer);
    if (value && !document.hidden)
      void (homeVisible ? loadRecent() : refresh()).finally(schedule);
  }
  const onVisibility = () => {
    if (document.hidden) clearTimeout(timer);
    else if (visible)
      void (homeVisible ? loadRecent() : refresh()).finally(schedule);
  };
  document.addEventListener("visibilitychange", onVisibility);
  function dispose() {
    alive = false;
    invalidate();
    clearTimeout(timer);
    document.removeEventListener("visibilitychange", onVisibility);
  }
  const lookup = (reference: TaskRef) => state.byRef.get(taskKey(reference));
  return {
    recentItems,
    loadRecent,
    restartRecent,
    lookup,
    port,
    tasks,
    state,
    preparations,
    available: !!tasks,
    interactive: !!port,
    load,
    previousPage,
    refresh,
    detail,
    openTask,
    closeTask,
    openNode: openTask,
    control,
    cancel,
    confirmPreparation,
    reconcileControl,
    setQuery,
    setVisible,
    dispose,
  };
}
export type Controller = ReturnType<typeof createTaskCenter>;
