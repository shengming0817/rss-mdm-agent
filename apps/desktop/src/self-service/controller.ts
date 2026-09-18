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
    snapshot.requests = [
      ...snapshot.requests.filter(
        (r) => r.plan.requestId !== task.plan.requestId,
      ),
      task,
    ];
    state.taskId = task.plan.requestId;
    if (task.plan.requestId === state.requestId) {
      state.accepted = true;
      state.uncertain = false;
    }
  }
  async function refresh() {
    if (!port) return;
    const sequence = ++refreshSequence;
    const writes = snapshotWrites;
    try {
      const snapshot = await port.snapshot();
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
        setError("测试服务已重启；旧测试数据已清空，请明确新建请求。");
      }
      if (errorSource === "snapshot") setError("");
      state.snapshot = snapshot;
      const task = snapshot.requests.find(
        (r) => r.plan.requestId === state.requestId,
      );
      if (task) {
        record(task);
        if (errorSource === "submission") setError("");
      }
      if (pendingReply) {
        const interaction = snapshot.requests
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
    }
  }
  function select(item: CatalogItem, fresh = false) {
    if (state.busy || state.uncertain) return;
    if (!fresh && state.item?.itemId === item.itemId) {
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
      state.uncertain
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
    for (const [key, value] of state.fields)
      if (value.kind === "secretReference") state.fields.delete(key);
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
    select,
    change,
    prepare,
    submit,
    respond,
    retryReply: sendReply,
    navigate,
  };
}
export type Controller = ReturnType<typeof createController>;
