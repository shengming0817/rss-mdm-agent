import { reactive } from "vue";
import type {
  Answer,
  CatalogItem,
  FieldInput,
  Plan,
  Reply,
  RequestView,
  SelfServicePort,
  Snapshot,
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
    plan: null as Plan | null,
    busy: false,
    error: "",
    uncertain: false,
    accepted: false,
    taskId: "",
    replying: false,
    replyUnknown: false,
    after: null as string | null,
    pageHistory: [] as (string | null)[],
    loading: false,
  });
  let generation = 0;
  let snapshotWrites = 0;
  let refreshSequence = 0;
  let pendingReply: Reply | null = null;
  let errorSource: "snapshot" | "submission" | "reply" | null = null;
  function setError(message: string, source: typeof errorSource = null) {
    state.error = message;
    errorSource = source;
  }
  function record(task: RequestView) {
    snapshotWrites++;
    const snapshot = state.snapshot;
    if (!snapshot) return;
    snapshot.requests = snapshot.requests.map((r) =>
      r.plan.requestId === task.plan.requestId ? task : r,
    );
    snapshot.referencedRequests = [task];
    state.taskId = task.plan.requestId;
    if (task.plan.requestId === state.requestId) {
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
      const submitted =
        state.accepted ||
        state.uncertain ||
        (state.busy && state.plan !== null);
      if (!submitted) {
        generation++;
        state.plan = null;
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
          pendingReply?.requestId ?? "",
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
        state.plan = null;
        state.item = null;
        state.requestId = "";
        state.uncertain = false;
        state.busy = false;
        state.replying = false;
        state.accepted = false;
        pendingReply = null;
        state.replyUnknown = false;
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
      const task = tasks.find((r) => r.plan.requestId === state.requestId);
      if (task) {
        state.accepted = true;
        state.uncertain = false;
        if (errorSource === "submission") setError("");
      }
      if (pendingReply) {
        const interaction = tasks
          .find((r) => r.plan.requestId === pendingReply?.requestId)
          ?.interactions.find((i) => i.id === pendingReply?.interactionId);
        if (interaction && interaction.status !== "pending") {
          pendingReply = null;
          state.replyUnknown = false;
          if (errorSource === "reply") setError("");
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
    state.plan = null;
    state.requestId = port ? newId() : "";
    state.revision = 1;
    state.accepted = false;
    setError("");
  }
  function change(key: string, value: FieldInput | null) {
    if (state.busy || state.accepted || state.uncertain) return;
    if (value === null) state.fields.delete(key);
    else state.fields.set(key, value);
    state.requestId = port ? newId() : "";
    state.revision++;
    state.plan = null;
    generation++;
  }
  async function prepare() {
    if (
      !port ||
      !state.item ||
      !state.snapshot ||
      state.busy ||
      state.accepted ||
      state.uncertain ||
      state.item.availability !== "listed" ||
      state.item.display.requestability !== "allowed"
    )
      return;
    const token = ++generation;
    state.busy = true;
    setError("");
    state.plan = null;
    try {
      const plan = await port.preview({
        instanceId: state.snapshot.instanceId,
        requestId: state.requestId,
        revision: state.revision,
        catalog: state.item.catalog,
        itemId: state.item.itemId,
        variantId: state.item.variantId,
        fields: Object.fromEntries(state.fields),
      });
      if (token === generation) state.plan = plan;
    } catch (error) {
      if (token === generation)
        setError(
          serviceError(error)
            ? error.message
            : "预览响应未收到，可按原请求重试。",
        );
    } finally {
      if (token === generation) state.busy = false;
    }
  }
  function clearSecrets() {
    let redacted = false;
    for (const [key, value] of state.fields) {
      if (value.kind === "secretReference") {
        state.fields.delete(key);
        redacted = true;
      }
    }
    // Redaction changes the next draft, not the frozen plan used by submit retries.
    if (redacted) state.revision++;
  }
  async function submit() {
    if (!port || !state.plan || !state.snapshot || state.busy || state.accepted)
      return;
    state.busy = true;
    setError("");
    const plan = state.plan;
    const token = ++generation;
    clearSecrets();
    try {
      const result = await port.submit({
        instanceId: state.snapshot.instanceId,
        requestId: plan.requestId,
        planId: plan.planId,
        digest: plan.digest,
      });
      if (token !== generation) return;
      record(result);
      state.page = "tasks";
    } catch (error) {
      if (token !== generation) return;
      if (serviceError(error)) {
        setError(error.message);
        state.uncertain = false;
        state.plan = null;
      } else {
        state.uncertain = true;
        setError(
          "提交响应未收到，结果尚不明确。请查询任务或按原请求重试，不要新建重复请求。",
          "submission",
        );
      }
    } finally {
      if (token === generation) state.busy = false;
    }
  }
  async function cancel(task: RequestView) {
    if (!port || !state.snapshot || state.busy) return;
    state.busy = true;
    try {
      record(
        await port.cancel({
          instanceId: state.snapshot.instanceId,
          requestId: task.plan.requestId,
          planId: task.plan.planId,
          digest: task.plan.digest,
        }),
      );
    } catch {
      setError("取消请求未确认；请查询原任务，不能据此认定已停止。");
    } finally {
      state.busy = false;
    }
  }
  async function approve(task: RequestView) {
    if (!port || !state.snapshot || state.busy || task.status !== "approval")
      return;
    state.busy = true;
    try {
      record(
        await port.approve({
          instanceId: state.snapshot.instanceId,
          requestId: task.plan.requestId,
          planId: task.plan.planId,
          digest: task.plan.digest,
        }),
      );
    } catch {
      setError("批准未确认；请刷新原任务，不创建新执行请求。");
    } finally {
      state.busy = false;
    }
  }
  async function sendReply() {
    if (!port || !pendingReply || state.replying) return;
    state.replying = true;
    setError("");
    const reply = pendingReply;
    try {
      const result = await port.respond(reply);
      if (state.snapshot?.instanceId !== reply.instanceId) return;
      record(result);
      pendingReply = null;
      state.replyUnknown = false;
    } catch (error) {
      if (state.snapshot?.instanceId !== reply.instanceId) return;
      if (serviceError(error)) {
        setError(error.message);
        pendingReply = null;
        state.replyUnknown = false;
      } else {
        state.replyUnknown = true;
        setError("交互回答结果不明，请刷新或重试原回答。", "reply");
      }
    } finally {
      if (state.snapshot?.instanceId === reply.instanceId)
        state.replying = false;
    }
  }
  async function respond(
    task: RequestView,
    interactionId: string,
    answer: Answer,
  ) {
    if (!port || !state.snapshot || state.replying || pendingReply) return;
    pendingReply = {
      instanceId: state.snapshot.instanceId,
      requestId: task.plan.requestId,
      interactionId,
      commandId: newId(),
      answer,
    };
    await sendReply();
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
    prepare,
    submit,
    respond,
    approve,
    cancel,
    retryReply: sendReply,
    navigate,
  };
}
export type Controller = ReturnType<typeof createController>;
