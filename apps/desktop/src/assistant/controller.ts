import { computed, markRaw, reactive, shallowRef } from "vue";
import {
  ClientError,
  type ClientOptions,
  type RuntimeClient,
  type SessionView,
  type Command,
  type SessionPage,
} from "@rss-mdm-agent/ai-client";
import type { ExecutionTaskDetails } from "./execution-types";

type PermissionRequest = Parameters<
  NonNullable<ClientOptions["requestPermission"]>
>[0];
type PermissionResponse = Awaited<
  ReturnType<NonNullable<ClientOptions["requestPermission"]>>
>;
export interface AssistantServices {
  /** Authenticated assembly owns transport and caller scope. A new connection must never reuse another caller's client. */
  connect(
    options: ClientOptions,
    signal: AbortSignal,
  ): Promise<{ runtime: RuntimeClient; mode: "s1" | "live" }>;
  /** Separate authenticated execution-app read; tool output never feeds this port. */
  taskDetails?(
    operationRequestId: string,
    signal: AbortSignal,
  ): Promise<ExecutionTaskDetails>;
}
export interface PermissionView {
  id: string;
  request: PermissionRequest;
}
/** ACP kind is trusted presentation semantics; provider names are supplementary. */
export function permissionPresentation(
  kind: PermissionRequest["options"][number]["kind"],
) {
  const labels = {
    allow_once: { label: "允许一次", scope: "仅本次请求" },
    allow_always: { label: "始终允许", scope: "持续授权：包含后续匹配请求" },
    reject_once: { label: "拒绝一次", scope: "仅本次请求" },
    reject_always: { label: "始终拒绝", scope: "持续拒绝：包含后续匹配请求" },
  } satisfies Record<
    PermissionRequest["options"][number]["kind"],
    { label: string; scope: string }
  >;
  return Object.entries(labels).find(([key]) => key === kind)?.[1];
}
/** Bound UI settlement even when an injected service ignores its cancellation signal. */
async function bounded<T>(
  owner: AbortController,
  operation: () => Promise<T>,
): Promise<T> {
  let abort = () => {};
  const cancelled = new Promise<never>((_, reject) => {
    abort = () => reject(new ClientError("request_failed"));
    owner.signal.addEventListener("abort", abort, { once: true });
    if (owner.signal.aborted) abort();
  });
  const timer = setTimeout(() => owner.abort(), 15_000);
  try {
    return await Promise.race([cancelled, operation()]);
  } finally {
    clearTimeout(timer);
    owner.signal.removeEventListener("abort", abort);
  }
}
type SessionItem = Pick<SessionPage["items"][number], "namespace" | "status">;
type Pending = { command: Command; draft?: string };
export function createAssistant(
  services: AssistantServices | undefined,
  identity: () => string,
  now = () => Date.now(),
) {
  const runtime = shallowRef<RuntimeClient>();
  const clock = shallowRef(now());
  const timer = setInterval(() => {
    clock.value = now();
  }, 1000);
  let connectionOwner: AbortController | undefined;
  let detailsOwner: AbortController | undefined;
  const state = reactive({
    connection: "disconnected" as "disconnected" | "connecting" | "connected",
    mode: "live" as "s1" | "live",
    a2ui: false,
    selected: "",
    error: "",
    listError: "",
    createError: "",
    cleanupError: "",
    next: undefined as string | undefined,
    listing: false,
    opening: false,
    sessions: new Map<string, SessionItem>(),
    views: new Map<string, SessionView>(),
    drafts: new Map<string, string>(),
    pending: new Map<string, Pending>(),
    sending: new Set<string>(),
    errors: new Map<string, string>(),
    permissions: new Map<string, PermissionView>(),
    task: undefined as ExecutionTaskDetails | undefined,
    taskError: "",
    taskLoading: false,
  });
  let epoch = 0,
    taskEpoch = 0,
    stop = () => {};
  const callbacks = new Map<string, (response: PermissionResponse) => void>();
  const view = computed(() => state.views.get(state.selected));
  const draft = computed({
    get: () => state.drafts.get(state.selected) ?? "",
    set: (value: string) => {
      state.drafts.set(state.selected, value);
    },
  });
  const live = (v?: SessionView) =>
    !!v &&
    v.connection === "attached" &&
    v.sessionStatus === "active" &&
    state.connection === "connected";
  const commands = computed(() => Object.values(view.value?.commands ?? {}));
  const busy = computed(() =>
    commands.value.some(
      (c) =>
        c.command.input.type === "prompt" &&
        !["terminal", "invalidated"].includes(c.state),
    ),
  );
  const active = computed(() =>
    commands.value.find(
      (c) =>
        c.command.input.type === "prompt" &&
        c.dispatch &&
        !["terminal", "invalidated"].includes(c.state),
    ),
  );
  const canSend = computed(
    () =>
      live(view.value) &&
      !state.pending.has(state.selected) &&
      !state.sending.has(state.selected) &&
      (!busy.value || view.value?.capabilities.queue === "supported"),
  );
  const canSteer = computed(
    () =>
      live(view.value) &&
      active.value?.state === "running" &&
      active.value?.dispatch?.certainty === "submitted" &&
      !!active.value?.dispatch?.nativeRunId &&
      view.value?.capabilities.steer === "supported" &&
      !state.pending.has(state.selected) &&
      !state.sending.has(state.selected),
  );
  const canCancel = computed(
    () =>
      live(view.value) &&
      active.value?.state === "running" &&
      active.value?.dispatch?.certainty === "submitted" &&
      ["request_only", "terminal_acknowledged"].includes(
        view.value!.capabilities.cancellation,
      ) &&
      !commands.value.some(
        (c) =>
          c.command.input.type === "cancel" &&
          c.command.input.targetCommandId === active.value?.command.commandId &&
          !["terminal", "invalidated"].includes(c.state),
      ) &&
      !state.pending.has(state.selected) &&
      !state.sending.has(state.selected),
  );
  const sessionConnection = computed(() =>
    state.connection !== "connected"
      ? state.connection
      : (view.value?.connection ?? "connected"),
  );
  const resumeReason = computed(() => {
    const v = view.value;
    if (v?.sessionStatus === "retired") return "retired";
    if (!live(v)) return "not-attached";
    if (v!.capabilities.continuation === "unknown") return "unknown";
    if (v!.capabilities.continuation === "unsupported") return "unsupported";
    if (
      commands.value.some(
        (c) => c.state === "running" || c.state === "dispatching",
      )
    )
      return "run-active";
    return "available";
  });
  const canResume = computed(() => resumeReason.value === "available");
  function actionable(v: SessionView, id: string, at: number) {
    const item = Object.entries(v.interactions).find(
      ([key]) => key === id,
    )?.[1];
    const surfaces = Object.values(v.surfaces).filter(
      (surface) => surface.interactionId === id,
    );
    const supported = surfaces.length
      ? state.a2ui &&
        surfaces.some(
          (surface) =>
            surface.status === "active" && surface.generation === v.generation,
        )
      : v.capabilities.structuredQuestion === "supported";
    return (
      supported &&
      !!item &&
      live(v) &&
      item.status === "pending" &&
      item.generation === v.generation &&
      at <= item.expiresAtMs &&
      !state.pending.has(v.namespace.sessionId) &&
      !state.sending.has(v.namespace.sessionId)
    );
  }
  const actionableIds = computed(
    () =>
      new Map(
        [...state.views.values()].map((v) => [
          v.namespace.sessionId,
          Object.keys(v.interactions).filter((id) =>
            actionable(v, id, clock.value),
          ),
        ]),
      ),
  );
  const attention = computed(
    () =>
      state.permissions.size +
      [...actionableIds.value.values()].reduce(
        (count, ids) => count + ids.length,
        0,
      ),
  );
  const background = computed(() =>
    [...actionableIds.value]
      .filter(([id, ids]) => id !== state.selected && ids.length > 0)
      .map(([id]) => id),
  );
  const fail = (error: unknown) =>
    error instanceof ClientError ? error.code : "request_failed";
  function clearPermissions() {
    for (const settle of [...callbacks.values()])
      settle({ outcome: { outcome: "cancelled" } });
  }
  function requestPermission(
    request: PermissionRequest,
    signal: AbortSignal,
  ): Promise<PermissionResponse> {
    return new Promise((resolve) => {
      if (
        signal.aborted ||
        request.options.some((option) => !permissionPresentation(option.kind))
      ) {
        resolve({ outcome: { outcome: "cancelled" } });
        return;
      }
      const id = identity();
      const finish = (response: PermissionResponse) => {
        if (!callbacks.delete(id)) return;
        signal.removeEventListener("abort", abort);
        state.permissions.delete(id);
        resolve(response);
      };
      const abort = () => finish({ outcome: { outcome: "cancelled" } });
      callbacks.set(id, finish);
      state.permissions.set(id, { id, request });
      signal.addEventListener("abort", abort, { once: true });
    });
  }
  function permission(id: string, optionId?: string) {
    const item = state.permissions.get(id);
    if (
      !item ||
      (optionId &&
        !item.request.options.some(
          (o) => o.optionId === optionId && permissionPresentation(o.kind),
        ))
    )
      return;
    callbacks.get(id)?.(
      optionId
        ? { outcome: { outcome: "selected", optionId } }
        : { outcome: { outcome: "cancelled" } },
    );
  }
  function closeClient(client?: RuntimeClient, current = epoch) {
    try {
      client?.close();
    } catch {
      if (current === epoch) state.cleanupError = "cleanup_failed";
    }
  }
  function release() {
    const client = runtime.value;
    runtime.value = undefined;
    const disposeObserver = stop;
    stop = () => {};
    try {
      disposeObserver();
    } catch {
      state.cleanupError = "cleanup_failed";
    }
    clearPermissions();
    closeClient(client);
  }
  async function connect() {
    const current = ++epoch;
    connectionOwner?.abort();
    detailsOwner?.abort();
    const owner = new AbortController();
    connectionOwner = owner;
    state.cleanupError = "";
    release();
    taskEpoch++;
    state.task = undefined;
    state.taskLoading = false;
    state.taskError = "";
    state.views.clear();
    state.sessions.clear();
    state.drafts.clear();
    state.pending.clear();
    state.sending.clear();
    state.errors.clear();
    state.selected = "";
    state.next = undefined;
    state.error = "";
    state.listError = "";
    state.createError = "";
    state.listing = false;
    state.opening = false;
    state.connection = services ? "connecting" : "disconnected";
    if (!services) return;
    let candidate: RuntimeClient | undefined;
    try {
      const connected = await bounded(owner, async () => {
        const result = await services.connect(
          {
            a2ui: true,
            requestPermission: (request, signal) =>
              current === epoch && !owner.signal.aborted
                ? requestPermission(request, signal)
                : Promise.resolve({ outcome: { outcome: "cancelled" } }),
          },
          owner.signal,
        );
        if (owner.signal.aborted) {
          closeClient(result.runtime, current);
          throw new ClientError("request_failed");
        }
        candidate = result.runtime;
        const negotiated = await candidate.initialize();
        return { ...result, negotiated };
      });
      if (current !== epoch || owner.signal.aborted) {
        closeClient(candidate, current);
        return;
      }
      const negotiated = connected.negotiated;
      runtime.value = markRaw(connected.runtime);
      state.mode = connected.mode;
      state.a2ui = !!negotiated.a2ui;
      stop = connected.runtime.observe((next) => {
        if (current === epoch) {
          state.views.set(next.namespace.sessionId, next);
          state.sessions.set(next.namespace.sessionId, {
            namespace: next.namespace,
            status: next.sessionStatus,
          });
        }
      });
      state.connection = "connected";
      void connected.runtime.connection.closed.then(() => {
        if (current === epoch) {
          state.connection = "disconnected";
          clearPermissions();
        }
      });
      await list(false);
    } catch (error) {
      owner.abort();
      if (current === epoch) {
        if (runtime.value) release();
        else closeClient(candidate, current);
        clearPermissions();
        state.connection = "disconnected";
        state.error = fail(error);
      } else closeClient(candidate, current);
    } finally {
      if (connectionOwner === owner) connectionOwner = undefined;
    }
  }
  async function list(more = true) {
    if (!runtime.value || state.listing || (more && !state.next)) return;
    const current = epoch;
    state.listing = true;
    state.listError = "";
    try {
      const page = await runtime.value.listSessions({
        limit: 20,
        ...(more && state.next ? { continuation: state.next } : {}),
      });
      if (current !== epoch) return;
      for (const item of page.items)
        state.sessions.set(item.namespace.sessionId, {
          namespace: item.namespace,
          status:
            state.views.get(item.namespace.sessionId)?.sessionStatus ??
            item.status,
        });
      state.next = page.next;
    } catch (error) {
      if (current === epoch) state.listError = fail(error);
    } finally {
      if (current === epoch) state.listing = false;
    }
  }
  async function select(id: string) {
    if (!runtime.value || !state.sessions.has(id)) return;
    state.selected = id;
    if (state.views.get(id)?.connection === "attached") return;
    const current = epoch;
    state.errors.delete(id);
    try {
      const next = await runtime.value.restore(id);
      if (current === epoch) {
        state.views.set(id, next);
        state.errors.delete(id);
      }
    } catch (error) {
      if (current === epoch) state.errors.set(id, fail(error));
    }
  }
  async function create() {
    if (!runtime.value || state.opening) return;
    const current = epoch;
    state.opening = true;
    state.createError = "";
    try {
      const next = await runtime.value.createSession();
      if (current !== epoch) return;
      state.views.set(next.namespace.sessionId, next);
      state.selected = next.namespace.sessionId;
      await list(false);
    } catch (error) {
      if (current === epoch) state.createError = fail(error);
    } finally {
      if (current === epoch) state.opening = false;
    }
  }
  async function retry(id = state.selected) {
    const pending = state.pending.get(id),
      client = runtime.value;
    if (
      !pending ||
      !client ||
      !live(state.views.get(id)) ||
      state.sending.has(id)
    )
      return;
    const current = epoch;
    state.sending.add(id);
    state.errors.delete(id);
    try {
      await client.submit(pending.command);
      if (current !== epoch) return;
      state.pending.delete(id);
      if (pending.draft !== undefined && state.drafts.get(id) === pending.draft)
        state.drafts.set(id, "");
    } catch (error) {
      if (current !== epoch) return;
      const code = fail(error);
      state.errors.set(id, code);
      // Unknown acceptance retains the exact ID, deadline and payload. No new command on retry.
      if (
        ![
          "request_failed",
          "transport_closed",
          "transport_failed",
          "revision_conflict",
          "unavailable",
        ].includes(code)
      )
        state.pending.delete(id);
    } finally {
      if (current === epoch) state.sending.delete(id);
    }
  }
  async function send(
    input: Command["input"],
    originalDraft?: string,
    expiry = now() + 60_000,
  ) {
    const id = state.selected;
    if (!live(view.value) || state.pending.has(id) || state.sending.has(id))
      return;
    state.pending.set(id, {
      command: {
        schemaVersion: 3,
        kind: "command",
        sessionId: id,
        commandId: identity(),
        expiresAtMs: expiry,
        input,
      },
      ...(originalDraft === undefined ? {} : { draft: originalDraft }),
    });
    await retry(id);
  }
  async function prompt(policy: "queue_next" | "steer" = "queue_next") {
    if (
      !(policy === "steer" ? canSteer.value : canSend.value) ||
      !draft.value.trim()
    )
      return;
    await send(
      {
        type: "prompt",
        text: draft.value.trim(),
        policy,
        ...(policy === "steer"
          ? { targetRunId: active.value!.dispatch!.nativeRunId! }
          : {}),
      },
      draft.value,
    );
  }
  async function cancel() {
    if (!canCancel.value) return;
    const target = active.value!;
    await send({
      type: "cancel",
      targetCommandId: target.command.commandId,
      generation: view.value!.generation,
      ...(target.dispatch?.nativeRunId
        ? { nativeRunId: target.dispatch.nativeRunId }
        : {}),
    });
  }
  function answerable(id: string) {
    const v = view.value;
    return (
      !!v &&
      actionableIds.value.get(v.namespace.sessionId)?.includes(id) === true &&
      actionable(v, id, now()) &&
      v.capabilities.structuredQuestion === "supported" &&
      !Object.values(v.surfaces).some((s) => s.interactionId === id)
    );
  }
  async function respond(id: string, answers: Record<string, string>) {
    if (!answerable(id)) return;
    const item = Object.entries(view.value!.interactions).find(
      ([key]) => key === id,
    )![1];
    await send(
      {
        type: "respond",
        interactionId: id,
        generation: item.generation,
        ...(item.nativeRunId ? { nativeRunId: item.nativeRunId } : {}),
        answer: { answers },
      },
      undefined,
      Math.min(item.expiresAtMs, now() + 60_000),
    );
  }
  async function restore(resume = false) {
    const id = state.selected,
      client = runtime.value,
      current = epoch;
    if (!client || (resume && !canResume.value) || state.opening) return;
    state.opening = true;
    try {
      const next = await (resume ? client.resume(id) : client.restore(id));
      if (current === epoch) {
        state.views.set(id, next);
        state.errors.delete(id);
      }
    } catch (error) {
      if (current === epoch) state.errors.set(id, fail(error));
    } finally {
      if (current === epoch) state.opening = false;
    }
  }
  async function detach() {
    const id = state.selected,
      current = epoch;
    try {
      await runtime.value?.detach(id);
    } catch (error) {
      if (current === epoch) state.errors.set(id, fail(error));
    }
  }
  async function taskDetails(id: string) {
    const current = ++taskEpoch;
    detailsOwner?.abort();
    const owner = new AbortController();
    detailsOwner = owner;
    state.task = undefined;
    state.taskError = "";
    if (!services?.taskDetails) {
      state.taskError = "执行服务未连接";
      return;
    }
    state.taskLoading = true;
    try {
      const read = services.taskDetails.bind(services);
      const result = await bounded(owner, () => read(id, owner.signal));
      if (current === taskEpoch) state.task = result;
    } catch {
      if (current === taskEpoch) state.taskError = "无法读取授权执行详情";
    } finally {
      if (current === taskEpoch) state.taskLoading = false;
      if (detailsOwner === owner) detailsOwner = undefined;
    }
  }
  function dispose() {
    epoch++;
    taskEpoch++;
    connectionOwner?.abort();
    detailsOwner?.abort();
    clearInterval(timer);
    release();
    state.connection = "disconnected";
    state.opening = false;
    state.listing = false;
    state.taskLoading = false;
  }
  return {
    state,
    runtime,
    view,
    draft,
    busy,
    active,
    canSend,
    canSteer,
    canCancel,
    canResume,
    sessionConnection,
    resumeReason,
    background,
    attention,
    clock,
    connect,
    list,
    select,
    create,
    prompt,
    cancel,
    respond,
    answerable,
    retry,
    restore,
    detach,
    permission,
    taskDetails,
    dispose,
  };
}
export type AssistantController = ReturnType<typeof createAssistant>;
