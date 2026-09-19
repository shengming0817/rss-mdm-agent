import {
  agent,
  RequestError,
  type AgentConnection,
  type RequestPermissionRequest,
  type RequestPermissionResponse,
  type SessionUpdate,
  type StopReason,
  type Stream,
} from "@agentclientprotocol/sdk";
import {
  accessLimits,
  boundedStream,
  boundedJson,
  decode,
  extension,
  interactionCatalog,
  parseNegotiation,
  selectNegotiation,
  resolveSurfaceAction,
  validateSurface,
  type AccessUpdate,
  type Budget,
  type Caller,
  type Command,
  type Event,
  type HostPort,
  type Limits,
  type Negotiation,
  type Outcome,
  type Result,
  type Session,
  type SessionOptions,
  type Subscription,
  type WireRecord,
} from "@rss-mdm-agent/ai-contract";

export interface AccessOptions {
  host: HostPort;
  sessionOptions: SessionOptions;
  limits?: Limits;
  timeoutMs?: number;
  /** Independent upper bound for settling owned work after cancellation. */
  shutdownTimeoutMs?: number;
  now?: () => number;
  /** Closed, value-free diagnostics; never receives caller data or provider errors. */
  onDiagnostic?: (code: AccessDiagnostic) => void;
}
export type AccessDiagnostic =
  | "cleanup_timeout"
  | "subscription_ended"
  | "subscription_failed"
  | "sequence_gap"
  | "budget_exceeded"
  | "invalid_surface"
  | "notification_failed"
  | "resync_delivery_failed";
class PumpFailure extends Error {
  constructor(readonly code: AccessDiagnostic) {
    super(code);
  }
}
const fail = (code: string): never => {
  throw new RequestError(-32001, code, { code });
};
const value = <T>(result: Result<T>): T =>
  result.ok ? result.value : fail(result.error.code);
const sameCaller = (a: Caller, b: Caller) =>
  a.tenantId === b.tenantId &&
  a.principalId === b.principalId &&
  a.authorityId === b.authorityId;
const stopReasons: Record<Exclude<Outcome, "failed">, StopReason> = {
  completed: "end_turn",
  cancelled: "cancelled",
  refused: "refusal",
  max_tokens: "max_tokens",
  max_turn_requests: "max_turn_requests",
};
type Waiter = {
  resolve: (outcome: Outcome) => void;
  reject: (reason: unknown) => void;
};
interface Pump {
  controller: AbortController;
  attachmentId?: string;
  after: number;
  terminal: Set<string>;
  text: Map<string, string>;
  waiters: Map<string, Waiter>;
}
interface Peer {
  caller: Caller;
  connection: AgentConnection;
  initialized: boolean;
  selected?: Negotiation;
  pumps: Map<string, Pump>;
  resumes: Map<string, { attachmentId?: string }>;
}

/** ACP adapter only. Caller and Host come from the trusted composition root.
 * Closing a connection detaches its views/callback deliveries, never cancels a run. */
export function createAccessService(options: AccessOptions) {
  const { host } = options,
    peers = new Set<Peer>(),
    tasks = new Set<Promise<unknown>>(),
    lifetime = new AbortController(),
    permissions = new Set<{
      caller: Caller;
      sessionId: string;
      controller: AbortController;
    }>();
  let closing: Promise<void> | undefined;
  const own = <T>(task: Promise<T>): Promise<T> => {
    tasks.add(task);
    void task.then(
      () => tasks.delete(task),
      () => tasks.delete(task),
    );
    return task;
  };
  const diagnose = (code: AccessDiagnostic) => {
    try {
      options.onDiagnostic?.(code);
    } catch {
      /* diagnostics cannot break cleanup */
    }
  };
  const cancelPermissions = (caller: Caller, sessionId: string) => {
    for (const pending of permissions)
      if (sameCaller(pending.caller, caller) && pending.sessionId === sessionId)
        pending.controller.abort();
  };
  const limits = options.limits ?? accessLimits,
    timeoutMs = options.timeoutMs ?? 30_000,
    now = options.now ?? Date.now,
    shutdownTimeoutMs = options.shutdownTimeoutMs ?? 5_000;
  if (
    !Number.isSafeInteger(shutdownTimeoutMs) ||
    shutdownTimeoutMs < 0 ||
    shutdownTimeoutMs > 2_147_483_647
  )
    throw new RangeError("invalid shutdownTimeoutMs");
  const budget = (signal: AbortSignal): Budget => ({
    signal: AbortSignal.any([signal, AbortSignal.timeout(timeoutMs)]),
    timeoutMs,
  });
  const parse = <K extends WireRecord["kind"]>(kind: K) => ({
    parse(input: unknown): Extract<WireRecord, { kind: K }> {
      try {
        const row = decode(boundedJson(input, limits), limits);
        if (row.kind === kind) return row as Extract<WireRecord, { kind: K }>;
      } catch {
        /* sanitized below */
      }
      return fail("invalid_input");
    },
  });
  const ready = (peer: Peer, product = false) => {
    if (lifetime.signal.aborted || !peer.initialized) fail("unavailable");
    if (product && !peer.selected) fail("unsupported_capability");
  };
  const session = async (
    peer: Peer,
    id: string,
    signal: AbortSignal,
  ): Promise<Session> =>
    value(
      await host.snapshotPage(peer.caller, id, { limit: 1 }, budget(signal)),
    ).session;
  const standard = async (
    peer: Peer,
    sessionId: string,
    update: SessionUpdate,
  ) => {
    try {
      boundedJson(update, limits);
    } catch {
      throw new PumpFailure("budget_exceeded");
    }
    try {
      await peer.connection.client.notify("session/update", {
        sessionId,
        update,
      });
    } catch {
      throw new PumpFailure("notification_failed");
    }
  };
  async function project(
    peer: Peer,
    id: string,
    pump: Pump,
    item: Subscription,
  ): Promise<void> {
    if (item.type === "resync_required") {
      for (const waiter of pump.waiters.values())
        waiter.reject(new RequestError(-32001, "cursor_expired"));
      pump.waiters.clear();
      return;
    }
    if (item.type === "delta") {
      if (pump.terminal.has(item.commandId)) return;
      const key = `${item.commandId}/${item.messageId}`;
      pump.text.set(key, (pump.text.get(key) ?? "") + item.text);
      if (
        pump.text.size > 1024 ||
        [...pump.text.values()].reduce(
          (n, text) => n + new TextEncoder().encode(text).byteLength,
          0,
        ) > limits.maxTextBytes
      )
        throw new PumpFailure("budget_exceeded");
      await standard(peer, id, {
        sessionUpdate: "agent_message_chunk",
        content: { type: "text", text: item.text },
      });
      return;
    }
    const event = item.event,
      body = event.body;
    if (event.commandId === undefined) return;
    if (body.type === "invalidated") {
      pump.terminal.add(event.commandId);
      pump.waiters
        .get(event.commandId)
        ?.reject(new RequestError(-32001, body.failure.code));
      pump.waiters.delete(event.commandId);
      for (const key of pump.text.keys())
        if (key.startsWith(`${event.commandId}/`)) pump.text.delete(key);
    } else if (body.type === "terminal") {
      pump.terminal.add(event.commandId);
      for (const key of pump.text.keys())
        if (key.startsWith(`${event.commandId}/`)) pump.text.delete(key);
      pump.waiters.get(event.commandId)?.resolve(body.outcome);
      pump.waiters.delete(event.commandId);
    } else if (body.type === "text") {
      const key = `${event.commandId}/${body.messageId}`,
        transient = pump.text.get(key) ?? "";
      const text = body.text.startsWith(transient)
        ? body.text.slice(transient.length)
        : body.text;
      pump.text.delete(key);
      if (text)
        await standard(peer, id, {
          sessionUpdate: "agent_message_chunk",
          content: { type: "text", text },
        });
    } else if (body.type === "tool_proposal") {
      await standard(peer, id, {
        sessionUpdate: "tool_call",
        toolCallId: body.proposalId,
        title: body.name,
        status: "pending",
        rawInput: body.arguments,
      });
    } else if (body.type === "tool_result") {
      await standard(peer, id, {
        sessionUpdate: "tool_call_update",
        toolCallId: body.proposalId,
        status: body.disposition === "returned" ? "completed" : "failed",
        content: [
          { type: "content", content: { type: "text", text: body.text } },
        ],
      });
    } else if (body.type === "interaction" && body.status === "pending") {
      // Ordinary questions stay ordinary questions; they are not ACP tool permission.
      await standard(peer, id, {
        sessionUpdate: "agent_message_chunk",
        content: { type: "text", text: JSON.stringify(body.request) },
      });
    }
  }
  const stopPump = (peer: Peer, id: string) => {
    peer.resumes.delete(id);
    const pump = peer.pumps.get(id);
    if (!pump) return;
    pump.controller.abort();
    for (const waiter of pump.waiters.values())
      waiter.reject(new RequestError(-32001, "unavailable"));
    pump.waiters.clear();
    peer.pumps.delete(id);
  };
  const startPump = (
    peer: Peer,
    id: string,
    after: number,
    attachmentId?: string,
  ): Pump => {
    stopPump(peer, id);
    const pump: Pump = {
      controller: new AbortController(),
      attachmentId,
      after,
      terminal: new Set(),
      text: new Map(),
      waiters: new Map(),
    };
    peer.pumps.set(id, pump);
    own(
      (async () => {
        try {
          for await (const item of host.subscribe(peer.caller, id, after, {
            signal: pump.controller.signal,
            timeoutMs,
          })) {
            if (pump.controller.signal.aborted) break;
            if (item.type === "event") {
              if (item.event.sequence <= pump.after) continue;
              if (item.event.sequence !== pump.after + 1)
                throw new PumpFailure("sequence_gap");
              pump.after = item.event.sequence;
              if (item.event.body.type === "surface" && peer.selected?.a2ui) {
                try {
                  validateSurface(item.event.body.surface, limits);
                } catch {
                  throw new PumpFailure("invalid_surface");
                }
              }
            }
            if (item.type === "delta" && pump.terminal.has(item.commandId))
              continue;
            if (pump.attachmentId) {
              const update: AccessUpdate = {
                schemaVersion: 3,
                kind: "accessUpdate",
                sessionId: id,
                attachmentId: pump.attachmentId,
                update: item,
              };
              parse("accessUpdate").parse(update);
              try {
                await peer.connection.client.notify(extension.update, update);
              } catch {
                throw new PumpFailure("notification_failed");
              }
            }
            await project(peer, id, pump, item);
            if (item.type === "resync_required") {
              if (!pump.attachmentId) peer.connection.close();
              return;
            }
          }
          if (!pump.controller.signal.aborted)
            throw new PumpFailure("subscription_ended");
        } catch (error) {
          if (!pump.controller.signal.aborted) {
            diagnose(
              error instanceof PumpFailure ? error.code : "subscription_failed",
            );
            if (pump.attachmentId) {
              try {
                await peer.connection.client.notify(extension.update, {
                  schemaVersion: 3,
                  kind: "accessUpdate",
                  sessionId: id,
                  attachmentId: pump.attachmentId,
                  update: { type: "resync_required" },
                });
              } catch {
                diagnose("resync_delivery_failed");
                peer.connection.close();
              }
            } else peer.connection.close();
          }
        } finally {
          if (peer.pumps.get(id) === pump) stopPump(peer, id);
        }
      })(),
    );
    return pump;
  };
  async function resume(
    peer: Peer,
    id: string,
    signal: AbortSignal,
  ): Promise<Session> {
    const previous = peer.pumps.get(id);
    stopPump(peer, id);
    const token = { attachmentId: previous?.attachmentId };
    peer.resumes.set(id, token);
    try {
      const resumed = value(await host.resume(peer.caller, id, budget(signal)));
      if (
        peer.resumes.get(id) !== token ||
        signal.aborted ||
        lifetime.signal.aborted ||
        peer.connection.signal.aborted
      )
        return fail("unavailable");
      if (previous?.attachmentId)
        await peer.connection.client.notify(extension.update, {
          schemaVersion: 3,
          kind: "accessUpdate",
          sessionId: id,
          attachmentId: previous.attachmentId,
          update: { type: "resync_required" },
        });
      if (
        peer.resumes.get(id) !== token ||
        signal.aborted ||
        lifetime.signal.aborted ||
        peer.connection.signal.aborted
      )
        return fail("unavailable");
      // A product client which detached first restores its own new attachment.
      if (previous || !peer.selected)
        startPump(peer, id, resumed.lastSequence, previous?.attachmentId);
      return resumed;
    } finally {
      if (peer.resumes.get(id) === token) peer.resumes.delete(id);
    }
  }
  async function submit(peer: Peer, command: Command, signal: AbortSignal) {
    const method =
      command.input.type === "prompt"
        ? "submit"
        : command.input.type === "cancel"
          ? "cancel"
          : "respond";
    return value(await host[method](peer.caller, command, budget(signal)));
  }
  function connect(stream: Stream, caller: Caller): AgentConnection {
    if (lifetime.signal.aborted) throw new Error("access service closed");
    const peer = {
      caller: Object.freeze({ ...caller }),
      initialized: false,
      pumps: new Map(),
      resumes: new Map(),
    } as Peer;
    const app = agent({ name: "rss-mdm-agent-ai-access" });
    app.onRequest("initialize", ({ params }) => {
      if (peer.initialized) return fail("content_conflict");
      if (params.protocolVersion !== 1) return fail("unsupported_version");
      const offer = params.clientCapabilities?._meta?.[extension.capability];
      if (offer !== undefined) {
        boundedJson(offer, limits);
        let n: Negotiation;
        try {
          n = parseNegotiation(offer, limits);
        } catch {
          return fail("unsupported_version");
        }
        if (
          !n ||
          n.contractVersion !== 3 ||
          n.acp !== 1 ||
          typeof n.cursorAttach !== "boolean" ||
          typeof n.durableReceipts !== "boolean"
        )
          return fail("unsupported_version");
        if (
          n.a2ui &&
          (n.a2ui.version !== interactionCatalog.version ||
            n.a2ui.catalogId !== interactionCatalog.catalogId ||
            n.a2ui.catalogVersion !== interactionCatalog.catalogVersion)
        )
          return fail("unsupported_capability");
        const selection = value(host.negotiate(structuredClone(n)));
        try {
          peer.selected = selectNegotiation(n, selection, limits);
        } catch {
          return fail("unsupported_capability");
        }
      }
      peer.initialized = true;
      return {
        protocolVersion: 1,
        agentCapabilities: {
          loadSession: true,
          promptCapabilities: {},
          sessionCapabilities: { list: {}, resume: {} },
          ...(peer.selected
            ? { _meta: { [extension.capability]: peer.selected } }
            : {}),
        },
        agentInfo: { name: "rss-mdm-agent-ai-access", version: "0.1.0" },
        authMethods: [],
      };
    });
    app.onRequest("session/new", async ({ params, signal }) => {
      ready(peer);
      if (params.mcpServers.length) return fail("unsupported_capability");
      const created = value(
        await host.createSession(
          peer.caller,
          options.sessionOptions,
          budget(signal),
        ),
      );
      if (!peer.selected) startPump(peer, created.namespace.sessionId, 0);
      return { sessionId: created.namespace.sessionId };
    });
    app.onRequest("session/list", async ({ params, signal }) => {
      ready(peer);
      const page = value(
        await host.listSessions(
          peer.caller,
          {
            limit: 64,
            ...(params.cursor ? { continuation: params.cursor } : {}),
          },
          budget(signal),
        ),
      );
      return {
        sessions: page.items.map((s) => ({
          sessionId: s.namespace.sessionId,
          cwd: "/",
        })),
        ...(page.next ? { nextCursor: page.next } : {}),
      };
    });
    app.onRequest("session/load", async ({ params, signal }) => {
      ready(peer);
      if (params.mcpServers.length) return fail("unsupported_capability");
      stopPump(peer, params.sessionId);
      let continuation: string | undefined,
        cursor = 0;
      const replay: Pump = {
        controller: new AbortController(),
        after: 0,
        terminal: new Set(),
        text: new Map(),
        waiters: new Map(),
      };
      do {
        const page = value(
          await host.snapshotPage(
            peer.caller,
            params.sessionId,
            { limit: 64, ...(continuation ? { continuation } : {}) },
            budget(signal),
          ),
        );
        cursor = page.cursor;
        for (const event of page.events)
          await project(peer, params.sessionId, replay, {
            type: "event",
            event,
          });
        continuation = page.next;
      } while (continuation);
      const pump = startPump(peer, params.sessionId, cursor);
      pump.terminal = replay.terminal;
      return {};
    });
    app.onRequest("session/resume", async ({ params, signal }) => {
      ready(peer);
      await resume(peer, params.sessionId, signal);
      return {};
    });
    app.onRequest("session/prompt", async ({ params, signal }) => {
      ready(peer);
      boundedJson(params, limits);
      await session(peer, params.sessionId, signal);
      const text = params.prompt
        .map((block) => {
          if (block.type === "text") return block.text;
          if (block.type === "resource_link")
            return `[Resource reference: ${block.name}; URI: ${block.uri}]`;
          return fail("unsupported_capability");
        })
        .join("\n");
      const command = parse("command").parse({
        schemaVersion: 3,
        kind: "command",
        sessionId: params.sessionId,
        commandId: crypto.randomUUID(),
        expiresAtMs: now() + timeoutMs,
        input: { type: "prompt", policy: "queue_next", text },
      });
      const pump =
        peer.pumps.get(params.sessionId) ??
        startPump(peer, params.sessionId, 0);
      const controller = signal;
      let onAbort: () => void = () => {};
      const terminal = new Promise<Outcome>((resolve, reject) => {
        onAbort = () => reject(new RequestError(-32001, "unavailable"));
        controller.addEventListener("abort", onAbort, { once: true });
        pump.waiters.set(command.commandId, { resolve, reject });
      });
      // Observe rejection even when admission fails before the terminal is awaited.
      void terminal.catch(() => {});
      try {
        await submit(peer, command, signal);
        const outcome = await terminal;
        if (outcome === "failed") return fail("unavailable");
        return { stopReason: stopReasons[outcome] };
      } finally {
        pump.waiters.delete(command.commandId);
        controller.removeEventListener("abort", onAbort);
      }
    });
    app.onNotification("session/cancel", async ({ params, signal }) => {
      ready(peer);
      const s = await session(peer, params.sessionId, signal);
      cancelPermissions(peer.caller, params.sessionId);
      let continuation: string | undefined;
      const accepted: string[] = [],
        turns = new Map<
          string,
          { commandId: string; policy: "queue_next" | "steer" }
        >();
      do {
        const page = value(
          await host.snapshotPage(
            peer.caller,
            params.sessionId,
            { limit: 64, ...(continuation ? { continuation } : {}) },
            budget(signal),
          ),
        );
        for (const record of page.commands) {
          if (
            record.command.input.type !== "prompt" ||
            record.state === "terminal" ||
            record.state === "invalidated"
          )
            continue;
          const dispatch = record.dispatch;
          if (!dispatch) {
            if (record.state === "accepted")
              accepted.push(record.command.commandId);
            continue;
          }
          if (
            dispatch.observerGeneration !== s.binding.generation ||
            dispatch.nativeSessionId !== s.binding.nativeSessionId ||
            dispatch.nativeThreadId !== s.binding.nativeThreadId ||
            dispatch.nativeRunId !== s.binding.nativeRunId
          )
            continue;
          const key = JSON.stringify([
              dispatch.nativeSessionId,
              dispatch.nativeThreadId ?? null,
              dispatch.nativeRunId ?? null,
            ]),
            current = turns.get(key),
            candidate = {
              commandId: record.command.commandId,
              policy: record.command.input.policy,
            };
          if (
            !current ||
            (current.policy === "steer" && candidate.policy === "queue_next")
          )
            turns.set(key, candidate);
        }
        continuation = page.next;
      } while (continuation);
      for (const targetCommandId of [
        ...accepted,
        ...[...turns.values()].map((turn) => turn.commandId),
      ]) {
        await submit(
          peer,
          {
            schemaVersion: 3,
            kind: "command",
            sessionId: params.sessionId,
            commandId: crypto.randomUUID(),
            expiresAtMs: now() + timeoutMs,
            input: {
              type: "cancel",
              targetCommandId,
              generation: s.binding.generation,
              ...(s.binding.nativeRunId
                ? { nativeRunId: s.binding.nativeRunId }
                : {}),
            },
          },
          signal,
        );
      }
    });
    app.onRequest(
      extension.submit,
      parse("command"),
      async ({ params, signal }) => {
        ready(peer, true);
        return submit(peer, params, signal);
      },
    );
    app.onRequest(
      extension.snapshot,
      parse("snapshotRequest"),
      async ({ params, signal }) => {
        ready(peer, true);
        const page = value(
          await host.snapshotPage(
            peer.caller,
            params.sessionId,
            params.query,
            budget(signal),
          ),
        );
        for (const surface of page.surfaces) validateSurface(surface, limits);
        return parse("snapshotPage").parse(page);
      },
    );
    app.onRequest(
      extension.list,
      parse("listRequest"),
      async ({ params, signal }) => {
        ready(peer, true);
        return value(
          await host.listSessions(peer.caller, params.query, budget(signal)),
        );
      },
    );
    app.onRequest(
      extension.attach,
      parse("attachRequest"),
      async ({ params, signal }) => {
        ready(peer, true);
        if (!peer.selected?.cursorAttach) return fail("unsupported_capability");
        await session(peer, params.sessionId, signal);
        startPump(peer, params.sessionId, params.after, params.attachmentId);
        return { ...params, kind: "attachReceipt" };
      },
    );
    app.onRequest(extension.detach, parse("detachRequest"), ({ params }) => {
      ready(peer, true);
      if (
        peer.pumps.get(params.sessionId)?.attachmentId ===
          params.attachmentId ||
        peer.resumes.get(params.sessionId)?.attachmentId === params.attachmentId
      )
        stopPump(peer, params.sessionId);
      return {};
    });
    app.onRequest(
      extension.resume,
      parse("resumeRequest"),
      async ({ params, signal }) => {
        ready(peer, true);
        return resume(peer, params.sessionId, signal);
      },
    );
    app.onRequest(
      extension.action,
      parse("actionRequest"),
      async ({ params, signal }) => {
        ready(peer, true);
        if (!peer.selected?.a2ui) return fail("unsupported_capability");
        const { metadata } = params;
        const resolver = {
          surface: async (_namespace: unknown, instance: string) => {
            const result = await host.surface(
              peer.caller,
              metadata.sessionId,
              instance,
              budget(signal),
            );
            if (result.ok) validateSurface(result.value, limits);
            return result;
          },
        };
        const resolved = value(
          await resolveSurfaceAction(
            resolver,
            peer.caller,
            metadata,
            params.message,
            limits,
          ),
        );
        return submit(
          peer,
          {
            schemaVersion: 3,
            kind: "command",
            sessionId: metadata.sessionId,
            commandId: metadata.commandId,
            expiresAtMs: params.expiresAtMs,
            input: {
              type: "respond",
              interactionId: resolved.interactionId,
              generation: metadata.generation,
              nativeRunId: metadata.nativeRunId,
              answer: resolved.answer,
              surface: resolved.surface,
            },
          },
          signal,
        );
      },
    );
    peer.connection = app.connect(boundedStream(stream, limits));
    peers.add(peer);
    void peer.connection.closed.then(() => {
      for (const id of peer.pumps.keys()) stopPump(peer, id);
      peers.delete(peer);
    });
    return peer.connection;
  }
  /** Delivery to attached clients for one live, composition-owned tool callback.
   * First valid answer wins. The callback owner supplies expiry/cancellation and
   * decides tool policy; this response cannot create an execution permit. */
  function requestPermission(
    caller: Caller,
    request: RequestPermissionRequest,
    signal: AbortSignal,
  ): Promise<RequestPermissionResponse> {
    return own(deliverPermission(caller, request, signal));
  }
  async function deliverPermission(
    caller: Caller,
    request: RequestPermissionRequest,
    signal: AbortSignal,
  ): Promise<RequestPermissionResponse> {
    boundedJson(request, limits);
    const recipients = [...peers].filter(
      (peer) =>
        peer.initialized &&
        sameCaller(peer.caller, caller) &&
        peer.pumps.has(request.sessionId),
    );
    if (!recipients.length || signal.aborted || lifetime.signal.aborted)
      return { outcome: { outcome: "cancelled" } };
    const done = new AbortController(),
      delivery = AbortSignal.any([
        signal,
        done.signal,
        lifetime.signal,
        AbortSignal.timeout(timeoutMs),
      ]);
    const pending = { caller, sessionId: request.sessionId, controller: done };
    permissions.add(pending);
    try {
      return await Promise.any(
        recipients.map(async (peer) => {
          const answer = await peer.connection.client.request(
            "session/request_permission",
            request,
            {
              cancellationSignal: AbortSignal.any([
                delivery,
                peer.pumps.get(request.sessionId)!.controller.signal,
              ]),
            },
          );
          if (delivery.aborted) return fail("unavailable");
          const outcome = answer.outcome;
          if (
            outcome.outcome === "selected" &&
            !request.options.some((o) => o.optionId === outcome.optionId)
          )
            return fail("invalid_input");
          return answer;
        }),
      );
    } catch {
      return { outcome: { outcome: "cancelled" } };
    } finally {
      done.abort();
      permissions.delete(pending);
    }
  }
  return {
    connect,
    requestPermission,
    close: (): Promise<void> => {
      if (closing) return closing;
      lifetime.abort();
      const connections = [...peers];
      for (const peer of connections) {
        for (const id of peer.pumps.keys()) stopPump(peer, id);
        peer.connection.close();
      }
      closing = new Promise<void>((resolve) => {
        const timer = setTimeout(() => {
          diagnose("cleanup_timeout");
          resolve();
        }, shutdownTimeoutMs);
        void Promise.allSettled([
          ...tasks,
          ...connections.map((peer) => peer.connection.closed),
        ]).then(() => {
          clearTimeout(timer);
          resolve();
        });
      });
      return closing;
    },
  };
}
