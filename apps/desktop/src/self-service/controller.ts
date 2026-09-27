import { reactive } from "vue";
import type {
  CatalogItem,
  FieldInput,
  Draft,
  RequestView,
  SelfServicePort,
  Snapshot,
  ActionRef,
} from "./types";
function sameSelection(a: CatalogItem, b: CatalogItem) {
  return (
    a.itemId === b.itemId &&
    a.variantId === b.variantId &&
    a.catalog.digest === b.catalog.digest &&
    a.catalog.identity.id === b.catalog.identity.id &&
    a.catalog.identity.revision === b.catalog.identity.revision &&
    a.catalog.authority.kind === b.catalog.authority.kind &&
    a.catalog.authority.id === b.catalog.authority.id &&
    (a.catalog.authority.kind !== "enterprise" ||
      (b.catalog.authority.kind === "enterprise" &&
        a.catalog.authority.tenant === b.catalog.authority.tenant))
  );
}
function serviceError(
  error: unknown,
): error is { code: string; message: string } {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    typeof error.code === "string" &&
    "message" in error &&
    typeof error.message === "string"
  );
}
function rejected(error: unknown): error is { code: string; message: string } {
  return (
    serviceError(error) &&
    ![
      "outcomeUnknown",
      "confirmationUnknown",
      "execution",
      "unavailable",
    ].includes(error.code)
  );
}
export function createController(
  port: SelfServicePort | null,
  newId: () => string,
  preview: Snapshot,
) {
  const state = reactive({
    snapshot: port ? (null as Snapshot | null) : preview,
    page: "home",
    item: null as CatalogItem | null,
    fields: new Map<string, FieldInput>(),
    requestId: "",
    revision: 1,
    busy: false,
    error: "",
    uncertain: false,
    accepted: false,
    taskId: "",
    after: null as string | null,
    pageHistory: [] as (string | null)[],
    loading: false,
  });
  let generation = 0;
  let snapshotWrites = 0;
  let refreshSequence = 0;
  let pendingExecution: Draft | null = null;
  let pendingAction: { kind: "cancel" | "confirm"; input: ActionRef } | null =
    null;
  let errorSource: "snapshot" | "submission" | "action" | null = null;
  function setError(message: string, source: typeof errorSource = null) {
    state.error = message;
    errorSource = source;
  }
  function record(task: RequestView) {
    snapshotWrites++;
    const snapshot = state.snapshot;
    if (!snapshot) return;
    snapshot.requests = snapshot.requests.map((r) =>
      r.action.requestId === task.action.requestId ? task : r,
    );
    snapshot.referencedRequests = [task];
    state.taskId = task.action.requestId;
    if (task.action.requestId === state.requestId) {
      state.accepted = true;
      state.uncertain = false;
    }
  }
  function rebindSelection(snapshot: Snapshot) {
    const previous = state.item;
    if (!previous) return;
    const item = snapshot.catalog.find((item) => sameSelection(item, previous));
    state.item = item ?? null;
    if (
      !item ||
      item.availability !== "listed" ||
      item.display.requestability !== "allowed"
    ) {
      // A submission in flight or with a lost response retains its frozen retry identity.
      // Catalog expiry cannot tell us whether that request was already accepted.
      const submitted = state.accepted || state.uncertain || state.busy;
      if (!submitted) {
        generation++;

        state.busy = false;
      }
      if (!item) {
        state.fields.clear();
        if (state.page === "detail")
          state.page = submitted
            ? "tasks"
            : previous.kind === "software"
              ? "software"
              : "tools";
        setError("所选目录项目已变更或移除，请从最新目录重新选择。");
      }
    }
  }
  async function refresh(after = state.after, history = state.pageHistory) {
    if (!port) return;
    const sequence = ++refreshSequence;
    const writes = snapshotWrites;
    state.loading = true;
    try {
      const snapshot = await port.snapshot({
        after,
        requestIds: [
          state.taskId,
          state.uncertain ? state.requestId : "",
          pendingAction?.input.requestId ?? "",
        ].filter(
          (value, index, ids) => value !== "" && ids.indexOf(value) === index,
        ),
      });
      if (
        sequence !== refreshSequence ||
        (snapshot.instanceId === state.snapshot?.instanceId &&
          writes !== snapshotWrites)
      )
        return;
      if (state.snapshot && snapshot.instanceId !== state.snapshot.instanceId) {
        generation++;
        state.fields.clear();

        state.item = null;
        state.requestId = "";
        state.uncertain = false;
        state.busy = false;
        state.accepted = false;
        pendingAction = null;
        pendingExecution = null;
        state.taskId = "";
        after = null;
        history = [];
        if (state.page === "detail") state.page = "home";
        setError("测试服务已重启；旧测试数据已清空，请明确新建请求。");
      }
      if (errorSource === "snapshot") setError("");
      state.snapshot = snapshot;
      state.after = after;
      state.pageHistory = history;
      rebindSelection(snapshot);
      const tasks = [...snapshot.requests, ...snapshot.referencedRequests];
      const task = tasks.find((r) => r.action.requestId === state.requestId);
      if (task) {
        state.accepted = true;
        state.uncertain = false;
        if (errorSource === "submission") setError("");
      }
      if (pendingAction) {
        const { kind, input } = pendingAction;
        const current = tasks.find(
          (r) =>
            r.action.requestId === input.requestId &&
            r.action.digest === input.digest,
        );
        if (
          current &&
          (kind === "confirm"
            ? current.status !== "confirmation"
            : ["stopped", "complete"].includes(current.status))
        ) {
          pendingAction = null;
          if (errorSource === "action") setError("");
        }
      }
    } catch {
      if (sequence === refreshSequence && writes === snapshotWrites)
        setError(
          "无法读取桌面测试服务。请重试；不会切换为演示成功。",
          "snapshot",
        );
    } finally {
      if (sequence === refreshSequence) state.loading = false;
    }
  }
  async function nextPage() {
    if (state.loading || !state.snapshot?.next) return;
    await refresh(state.snapshot.next, [...state.pageHistory, state.after]);
  }
  async function previousPage() {
    if (state.loading || !state.pageHistory.length) return;
    await refresh(state.pageHistory.at(-1)!, state.pageHistory.slice(0, -1));
  }
  function select(item: CatalogItem, fresh = false) {
    if (state.busy || state.uncertain) return;
    if (!fresh && state.item && sameSelection(state.item, item)) {
      state.page = "detail";
      return;
    }
    generation++;
    state.item = item;
    state.page = "detail";
    state.fields.clear();

    state.requestId = port ? newId() : "";
    state.revision = 1;
    pendingExecution = null;
    state.accepted = false;
    setError("");
  }
  function change(key: string, value: FieldInput | null) {
    if (state.busy || state.accepted || state.uncertain) return;
    if (value === null) state.fields.delete(key);
    else state.fields.set(key, value);
    state.requestId = port ? newId() : "";
    state.revision++;
    pendingExecution = null;

    generation++;
  }
  async function execute() {
    if (
      !port ||
      !state.item ||
      !state.snapshot ||
      state.busy ||
      state.accepted ||
      pendingAction
    )
      return;
    if (!pendingExecution) {
      if (
        state.item.availability !== "listed" ||
        state.item.display.requestability !== "allowed"
      )
        return;
      pendingExecution = {
        instanceId: state.snapshot.instanceId,
        requestId: state.requestId,
        revision: state.revision,
        catalog: state.item.catalog,
        itemId: state.item.itemId,
        variantId: state.item.variantId,
        fields: Object.fromEntries(state.fields),
      };
    }
    state.busy = true;
    const token = ++generation;
    setError("");
    try {
      const result = await port.execute(pendingExecution);
      if (token !== generation) return;
      record(result);
      pendingExecution = null;
      state.fields.clear();
      state.page = "tasks";
    } catch (error) {
      if (token !== generation) return;
      if (rejected(error)) {
        pendingExecution = null;
        state.uncertain = false;
        setError(error.message);
      } else {
        state.uncertain = true;
        setError("执行请求结果未确认，请查询或重试原请求。", "submission");
      }
    } finally {
      if (token === generation) state.busy = false;
    }
  }
  async function act(kind: "cancel" | "confirm", task: RequestView) {
    if (
      !port ||
      !state.snapshot ||
      state.busy ||
      state.uncertain ||
      (kind === "confirm" && task.status !== "confirmation")
    )
      return;
    const input: ActionRef = {
      instanceId: state.snapshot.instanceId,
      requestId: task.action.requestId,
      digest: task.action.digest,
    };
    if (
      pendingAction &&
      (pendingAction.kind !== kind ||
        pendingAction.input.instanceId !== input.instanceId ||
        pendingAction.input.requestId !== input.requestId ||
        pendingAction.input.digest !== input.digest)
    )
      return;
    const pending = pendingAction ?? { kind, input };
    pendingAction = pending;
    state.busy = true;
    try {
      const result = await (kind === "cancel"
        ? port.cancel(pending.input)
        : port.confirm(pending.input));
      if (state.snapshot?.instanceId !== input.instanceId) return;
      record(result);
      pendingAction = null;
      setError("");
    } catch (error) {
      if (state.snapshot?.instanceId !== input.instanceId) return;
      if (rejected(error)) {
        pendingAction = null;
        setError(error.message);
      } else
        setError(
          kind === "cancel"
            ? "取消请求未确认；请查询原任务，不能据此认定已停止。"
            : "动作确认结果未收到；请刷新原任务，不创建新执行请求。",
          "action",
        );
    } finally {
      if (state.snapshot?.instanceId === input.instanceId) state.busy = false;
    }
  }
  async function cancel(task: RequestView) {
    await act("cancel", task);
  }
  async function confirm(task: RequestView) {
    await act("confirm", task);
  }
  function navigate(page: string) {
    state.page = page;
    if (page === "tasks") void refresh();
  }
  return {
    state,
    interactive: port !== null,
    refresh,
    nextPage,
    previousPage,
    select,
    change,
    execute,
    confirm,
    cancel,
    navigate,
  };
}
export type Controller = ReturnType<typeof createController>;
