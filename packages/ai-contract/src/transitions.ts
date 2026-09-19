import { createHash } from "node:crypto";
import {
  VerifiedProviderSession,
  reconciliationFor,
  providerIdentity,
} from "./session.js";
import canonicalize from "canonicalize";
import {
  decode,
  isId,
  boundedJson,
  fingerprint,
  deliveryFingerprint,
  ContractError,
  type Limits,
} from "./codec.js";
import type {
  AcceptCommand,
  Result,
  SessionCommit,
  Snapshot,
  SessionRebind,
  Reconciliation,
} from "./ports.js";
import type {
  CommandRecord,
  Counter,
  Delivery,
  Event,
  Id,
  Namespace,
  Session,
} from "./wire.js";
export const defaultLimits: Readonly<Limits> = Object.freeze({
  maxBytes: 262144,
  maxTextBytes: 131072,
  maxDepth: 32,
  maxNodes: 16384,
});
export const ok = <T>(value: T): Result<T> => ({ ok: true, value });
export const fail = <T = never>(
  code: import("./wire.js").ErrorCode,
  retry: import("./wire.js").Retry = "never",
): Result<T> => ({ ok: false, error: { code, retry } });
const clone = <T>(value: T): T => structuredClone(value);
export const namespaceKey = (n: Namespace): string => {
  const parts = [n?.tenantId, n?.principalId, n?.authorityId, n?.sessionId];
  if (parts.some((part) => !isId(part))) throw new ContractError("context");
  return JSON.stringify(parts);
};
const valid = (v: unknown, limits: Limits) =>
  decode(boundedJson(v, limits), limits);
const same = (a: unknown, b: unknown) => canonicalize(a) === canonicalize(b);
export interface SessionState {
  session: Session;
  generations: Set<Id>;
  commands: Map<Id, CommandRecord>;
  events: Event[];
  interactions: Map<Id, Snapshot["interactions"][number]>;
  deliveries: Map<Id, Delivery>;
  surfaces: Map<Id, Snapshot["surfaces"][number]>;
}

export function eventId(seed: string, label: string): Id {
  return createHash("sha256")
    .update(JSON.stringify([seed, label]))
    .digest("hex");
}
export const isSettled = (c: CommandRecord): boolean =>
  c.state === "terminal" || c.state === "invalidated";
export function createState(
  session: Session,
  limits: Limits = defaultLimits,
): Result<SessionState> {
  try {
    valid(session, limits);
    namespaceKey(session.namespace);
  } catch {
    return fail("invalid_input");
  }
  if (
    session.revision !== 0 ||
    session.lastSequence !== 0 ||
    session.status !== "active"
  )
    return fail("invalid_input");
  return ok({
    session: clone(session),
    generations: new Set([session.binding.generation]),
    commands: new Map(),
    events: [],
    interactions: new Map(),
    deliveries: new Map(),
    surfaces: new Map(),
  });
}
function reduceAcceptance(
  state: SessionState,
  input: AcceptCommand,
  limits: Limits = defaultLimits,
): Result<{ state: SessionState; receipt: import("./wire.js").Receipt }> {
  try {
    valid(input.command, limits);
    valid(input.event, limits);
  } catch {
    return fail("invalid_input");
  }
  if (input.command.sessionId !== input.namespace.sessionId)
    return fail("permission_denied");
  if (!Number.isSafeInteger(input.nowMs) || input.nowMs < 0)
    return fail("invalid_input");
  if (namespaceKey(input.namespace) !== namespaceKey(state.session.namespace))
    return fail("permission_denied");
  const digest = fingerprint(input.command, limits),
    prior = state.commands.get(input.command.commandId);
  if (prior) {
    if (prior.receipt.contentHash !== digest) return fail("content_conflict");
    return input.nowMs <= prior.receipt.receiptUntilMs
      ? ok({ state, receipt: clone(prior.receipt) })
      : fail("expired");
  }
  if (state.session.status !== "active") return fail("session_gone");
  if (
    !Number.isSafeInteger(input.nowMs) ||
    input.nowMs < 0 ||
    input.command.expiresAtMs < input.nowMs
  )
    return fail("expired");
  const { retryWindowMs, receiptWindowMs } = input.retention;
  if (
    !Number.isSafeInteger(retryWindowMs) ||
    retryWindowMs < 1 ||
    !Number.isSafeInteger(receiptWindowMs) ||
    receiptWindowMs < retryWindowMs ||
    !Number.isSafeInteger(input.nowMs + receiptWindowMs)
  )
    return fail("invalid_input");
  const check = checkState(
    state,
    input.expectedRevision,
    input.expectedGeneration,
  );
  if (!check.ok) return check;
  const receipt: import("./wire.js").Receipt = {
    schemaVersion: 2,
    kind: "receipt",
    namespace: clone(input.namespace),
    commandId: input.command.commandId,
    contentHash: digest,
    acceptedAtMs: input.nowMs,
    retryUntilMs: Math.min(
      input.command.expiresAtMs,
      input.nowMs + retryWindowMs,
    ),
    receiptUntilMs: input.nowMs + receiptWindowMs,
    acceptedRevision: state.session.revision + 1,
  };
  const record: CommandRecord = {
    schemaVersion: 2,
    kind: "commandRecord",
    command: clone(input.command),
    receipt,
    state: "accepted",
  };
  const interactions: Snapshot["interactions"][number][] = [];
  if (input.command.input.type === "respond") {
    const request = input.command.input;
    const interaction = state.interactions.get(request.interactionId);
    const linked = [...state.surfaces.values()].filter(
      (row) => row.interactionId === request.interactionId,
    );
    if (linked.length || request.surface) {
      const surface =
        request.surface && state.surfaces.get(request.surface.instanceId);
      if (
        !surface ||
        surface.interactionId !== request.interactionId ||
        surface.revision !== request.surface!.revision
      )
        return fail("stale_binding");
      if (surface.status !== "active") return fail("unavailable");
    }
    if (!interaction || interaction.status === "unavailable")
      return fail("unavailable");
    if (
      interaction.generation !== request.generation ||
      request.generation !== input.expectedGeneration ||
      interaction.nativeRunId !== request.nativeRunId
    )
      return fail("stale_binding");
    if (interaction.status === "answered") return fail("already_answered");
    if (
      interaction.status === "expired" ||
      interaction.expiresAtMs < input.nowMs
    )
      return fail("expired");
    interactions.push({
      ...interaction,
      status: "answered",
      responseCommandId: input.command.commandId,
    });
  }
  const result = reduceCommit(
    state,
    {
      namespace: input.namespace,
      expectedRevision: input.expectedRevision,
      expectedGeneration: input.expectedGeneration,
      session: {
        ...state.session,
        revision: state.session.revision + 1,
        lastSequence: state.session.lastSequence + 1 + interactions.length,
      },
      commands: [record],
      events: [
        input.event,
        ...interactions.map((row, i) => ({
          schemaVersion: 2 as const,
          kind: "event" as const,
          namespace: input.namespace,
          eventId: eventId(input.event.eventId, `answer-${i}`),
          sequence: state.session.lastSequence + 2 + i,
          commandId: row.commandId,
          attemptId: state.commands.get(row.commandId)!.dispatch!.attemptId,
          generation: row.generation,
          body: {
            type: "interaction" as const,
            interactionId: row.interactionId,
            status: "answered" as const,
          },
        })),
      ],
      interactions,
      deliveries: [],
      surfaces: [],
    },
    limits,
    true,
  );
  return result.ok
    ? ok({ state: result.value, receipt: clone(receipt) })
    : result;
}
export function checkState(
  state: SessionState,
  revision: Counter,
  generation: Id,
): Result<void> {
  if (state.session.status !== "active") return fail("session_gone");
  if (state.session.binding.generation !== generation)
    return fail("stale_binding");
  if (state.session.revision !== revision)
    return fail("revision_conflict", "same_command");
  return ok(undefined);
}

export function commitSession(
  state: SessionState,
  batch: SessionCommit,
  limits: Limits = defaultLimits,
): Result<SessionState> {
  return guarded(() => reduceCommit(state, batch, limits, false));
}
function reduceCommit(
  state: SessionState,
  batch: SessionCommit,
  limits: Limits,
  accepting: boolean,
): Result<SessionState> {
  const check = checkState(
    state,
    batch.expectedRevision,
    batch.expectedGeneration,
  );
  if (!check.ok) return check;
  try {
    for (const row of [
      batch.session,
      ...batch.commands,
      ...batch.events,
      ...batch.interactions,
      ...batch.deliveries,
      ...batch.surfaces,
    ])
      valid(row, limits);
  } catch (error) {
    if (error instanceof ContractError) return fail("invalid_input");
    throw error;
  }
  if (
    namespaceKey(state.session.namespace) !== namespaceKey(batch.namespace) ||
    namespaceKey(batch.session.namespace) !== namespaceKey(batch.namespace) ||
    batch.session.revision !== state.session.revision + 1 ||
    batch.session.lastSequence !==
      state.session.lastSequence + batch.events.length ||
    batch.session.status !== "active"
  )
    return fail("invalid_input");
  if (
    !same(
      providerIdentity(batch.session.binding),
      providerIdentity(state.session.binding),
    ) ||
    !same(batch.session.capabilities, state.session.capabilities)
  )
    return fail("stale_binding");
  const copy = clone(state);
  const commandIds = new Set<Id>();
  const attemptIds = new Set(
    state.events.flatMap((e) => (e.attemptId ? [e.attemptId] : [])),
  );
  const reconciliations = new Map<Id, Reconciliation>();
  for (const proof of batch.reconciliations ?? []) {
    const record = state.commands.get(proof?.commandId);
    const observation =
      record && reconciliationFor(proof, state.session, record);
    if (!observation) return fail("permission_denied");
    if (reconciliations.has(proof.commandId)) return fail("invalid_input");
    reconciliations.set(proof.commandId, observation);
  }
  for (const c of batch.commands) {
    const id = c.command.commandId;
    if (commandIds.has(id)) return fail("invalid_input");
    commandIds.add(id);
    if (
      namespaceKey(c.receipt.namespace) !== namespaceKey(batch.namespace) ||
      c.command.sessionId !== batch.namespace.sessionId
    )
      return fail("permission_denied");
    const old = state.commands.get(id),
      resolution = reconciliations.get(id);
    if (old && (!same(old.receipt, c.receipt) || !same(old.command, c.command)))
      return fail("content_conflict");
    if (old && isSettled(old) && !same(old, c)) return fail("content_conflict");
    if (
      !old &&
      (!accepting ||
        c.state !== "accepted" ||
        c.receipt.acceptedRevision !== batch.session.revision)
    )
      return fail("invalid_input");
    if (
      c.dispatch &&
      c.dispatch.observerGeneration !== batch.expectedGeneration
    )
      return fail("stale_binding");
    if (resolution) {
      if (
        !old?.dispatch ||
        isSettled(old) ||
        resolution.attemptId !== old.dispatch.attemptId ||
        !same(
          providerIdentity(resolution.binding),
          providerIdentity(state.session.binding),
        ) ||
        resolution.binding.nativeSessionId !== old.dispatch.nativeSessionId ||
        ["nativeRunId", "nativeRequestId"].some((key) => {
          const k = key as "nativeRunId" | "nativeRequestId";
          return (
            old.dispatch![k] !== undefined &&
            resolution.binding[k] !== old.dispatch![k]
          );
        })
      )
        return fail("stale_binding");
      const proof = batch.events.filter(
        (e) => e.commandId === id && e.body.type === "reconciled",
      );
      if (
        proof.length !== 1 ||
        proof[0].body.type !== "reconciled" ||
        proof[0].body.resolution !== resolution.status ||
        !same(proof[0].body.attempt, old.dispatch)
      )
        return fail("invalid_input");
      if (resolution.status === "not_submitted") {
        if (!Number.isSafeInteger(batch.nowMs) || batch.nowMs! < 0)
          return fail("invalid_input");
        if (old.dispatch.certainty === "submitted")
          return fail("content_conflict");
        const eligible =
          c.command.input.type === "prompt" &&
          c.command.input.policy === "queue_next" &&
          batch.nowMs! <=
            Math.min(c.command.expiresAtMs, c.receipt.retryUntilMs);
        if (
          c.state !== (eligible ? "accepted" : "invalidated") ||
          c.dispatch !== undefined
        )
          return fail("invalid_input");
      } else if (
        c.state !==
          (resolution.status === "unknown"
            ? "reconciliation_required"
            : resolution.status) ||
        (resolution.status === "terminal" && c.outcome !== resolution.outcome)
      )
        return fail("invalid_input");
    } else if (old && old.state !== c.state) {
      const allowed: Record<string, readonly string[]> = {
        accepted: ["dispatching", "invalidated"],
        dispatching: ["running", "terminal", "reconciliation_required"],
        running: ["terminal", "reconciliation_required"],
        reconciliation_required: [],
        terminal: [],
        invalidated: [],
      };
      if (!allowed[old.state].includes(c.state))
        return fail("content_conflict");
    }
    if (old?.dispatch && c.dispatch) {
      for (const key of [
        "attemptId",
        "originGeneration",
        "observerGeneration",
        "nativeSessionId",
        "nativeRunId",
        "nativeRequestId",
        "correlationId",
      ] as const)
        if (
          old.dispatch[key] !== undefined &&
          old.dispatch[key] !== c.dispatch[key]
        )
          return fail("stale_binding");
      if (
        old.dispatch.certainty === "submitted" &&
        c.dispatch.certainty !== "submitted"
      )
        return fail("content_conflict");
      if (
        old.dispatch.certainty !== "intent" &&
        c.dispatch.certainty === "intent"
      )
        return fail("content_conflict");
    } else if (c.dispatch) {
      if (
        !old ||
        old.state !== "accepted" ||
        c.state !== "dispatching" ||
        c.dispatch.certainty !== "intent" ||
        c.dispatch.originGeneration !== batch.expectedGeneration ||
        c.dispatch.nativeSessionId !== state.session.binding.nativeSessionId ||
        attemptIds.has(c.dispatch.attemptId)
      )
        return fail("invalid_input");
      if (
        !Number.isSafeInteger(batch.nowMs) ||
        batch.nowMs! < c.receipt.acceptedAtMs
      )
        return fail("invalid_input");
      if (batch.nowMs! > dispatchDeadline(state, c)) return fail("expired");
      attemptIds.add(c.dispatch.attemptId);
    } else if (old?.dispatch && resolution?.status !== "not_submitted")
      return fail("invalid_input");
    if (c.dispatch && (!old?.dispatch || !same(old.dispatch, c.dispatch))) {
      const events = batch.events.filter(
        (e) => e.commandId === id && e.body.type === "dispatch",
      );
      if (
        events.length !== 1 ||
        events[0].body.type !== "dispatch" ||
        !same(events[0].body.attempt, c.dispatch)
      )
        return fail("invalid_input");
    }
    if (c.dispatch?.certainty === "unknown" && !c.dispatch.correlationId)
      return fail("invalid_input");
    if (c.state === "running" && c.dispatch.certainty !== "submitted")
      return fail("invalid_input");
    if (c.state === "invalidated" && old?.state !== "invalidated") {
      if (
        c.failure.retry !== "never" ||
        !["expired", "stale_binding", "unavailable"].includes(c.failure.code) ||
        (resolution?.status !== "not_submitted" &&
          !(
            old?.state === "accepted" &&
            ((c.failure.code === "expired" &&
              Number.isSafeInteger(batch.nowMs) &&
              batch.nowMs! > dispatchDeadline(state, c)) ||
              (c.command.input.type !== "prompt" &&
                c.command.input.generation !== batch.expectedGeneration))
          )) ||
        !batch.events.some(
          (e) =>
            e.commandId === id &&
            e.body.type === "invalidated" &&
            same(e.body.failure, c.failure),
        )
      )
        return fail("invalid_input");
    }
    if (c.state === "terminal" && old?.state !== "terminal") {
      if (
        !old?.dispatch ||
        c.dispatch.certainty !== "submitted" ||
        !batch.events.some(
          (e) =>
            e.commandId === id &&
            e.attemptId === c.dispatch.attemptId &&
            e.generation === c.dispatch.observerGeneration &&
            e.body.type === "terminal" &&
            e.body.outcome === c.outcome,
        )
      )
        return fail("invalid_input");
    }
    if (
      (!old || old.state !== c.state) &&
      !["terminal", "invalidated"].includes(c.state) &&
      !batch.events.some(
        (e) =>
          e.commandId === id &&
          e.body.type === "status" &&
          e.body.state === c.state,
      )
    )
      return fail("invalid_input");
    copy.commands.set(id, clone(c));
  }
  if ([...reconciliations.keys()].some((id) => !commandIds.has(id)))
    return fail("invalid_input");
  const oldBinding = state.session.binding,
    nextBinding = batch.session.binding;
  const coordinatesMatch = (
    dispatch: CommandRecord["dispatch"],
    binding: Session["binding"],
  ) =>
    dispatch !== undefined &&
    dispatch.nativeRunId === binding.nativeRunId &&
    dispatch.nativeRequestId === binding.nativeRequestId;
  if (
    oldBinding.nativeRunId !== nextBinding.nativeRunId ||
    oldBinding.nativeRequestId !== nextBinding.nativeRequestId
  ) {
    const records = [...copy.commands.values()];
    if (
      nextBinding.nativeRunId !== undefined ||
      nextBinding.nativeRequestId !== undefined
    ) {
      if (
        !records.some(
          (c) =>
            (c.state === "dispatching" || c.state === "running") &&
            c.dispatch?.certainty === "submitted" &&
            coordinatesMatch(c.dispatch, nextBinding),
        )
      )
        return fail("stale_binding");
    } else {
      const original = records.filter((c) =>
        coordinatesMatch(c.dispatch, oldBinding),
      );
      if (
        !original.length ||
        original.some((c) => !isSettled(c) && c.state !== "accepted")
      )
        return fail("stale_binding");
    }
  }
  const ids = new Set(copy.events.map((e) => e.eventId));
  for (const [i, event] of batch.events.entries()) {
    if (
      namespaceKey(event.namespace) !== namespaceKey(batch.namespace) ||
      event.generation !== batch.expectedGeneration ||
      event.sequence !== state.session.lastSequence + i + 1 ||
      ids.has(event.eventId) ||
      event.commandId === undefined ||
      !copy.commands.has(event.commandId)
    )
      return fail("invalid_input");
    if (event.commandId === undefined) return fail("invalid_input");
    const source = copy.commands.get(event.commandId)!;
    const prior = state.commands.get(event.commandId);
    if (
      event.attemptId !== undefined &&
      (event.attemptId !== (source.dispatch ?? prior?.dispatch)?.attemptId ||
        (source.dispatch ?? prior?.dispatch)?.observerGeneration !==
          batch.expectedGeneration)
    )
      return fail("stale_binding");
    if (
      ["text", "tool_proposal", "tool_result", "surface"].includes(
        event.body.type,
      ) &&
      isSettled(source)
    )
      return fail("content_conflict");
    if (
      ["dispatch", "reconciled", "invalidated", "status"].includes(
        event.body.type,
      ) &&
      !commandIds.has(event.commandId)
    )
      return fail("invalid_input");
    if (event.body.type === "status" && event.body.state !== source.state)
      return fail("invalid_input");
    if (
      event.body.type === "dispatch" &&
      !same(event.body.attempt, source.dispatch)
    )
      return fail("invalid_input");
    if (
      event.body.type === "reconciled" &&
      !reconciliations.has(event.commandId)
    )
      return fail("invalid_input");
    if (
      event.body.type === "invalidated" &&
      (source.state !== "invalidated" ||
        !same(source.failure, event.body.failure))
    )
      return fail("invalid_input");
    if (
      event.body.type === "surface_invalidated" &&
      (!isSettled(source) ||
        !commandIds.has(event.commandId) ||
        (prior && isSettled(prior)))
    )
      return fail("invalid_input");
    if (event.body.type === "terminal") {
      const record = copy.commands.get(event.commandId)!;
      if (
        record.state !== "terminal" ||
        record.outcome !== event.body.outcome ||
        !record.dispatch ||
        !batch.commands.some((c) => c.command.commandId === event.commandId) ||
        state.commands.get(event.commandId)?.state === "terminal"
      )
        return fail("invalid_input");
    }
    ids.add(event.eventId);
    copy.events.push(clone(event));
  }
  const interactionIds = new Set<Id>();
  for (const row of batch.interactions) {
    if (interactionIds.has(row.interactionId)) return fail("invalid_input");
    interactionIds.add(row.interactionId);
    const source = copy.commands.get(row.commandId);
    if (
      namespaceKey(row.namespace) !== namespaceKey(batch.namespace) ||
      row.generation !== batch.expectedGeneration ||
      !source?.dispatch ||
      source.dispatch.observerGeneration !== row.generation ||
      source.dispatch.nativeRunId !== row.nativeRunId
    )
      return fail("stale_binding");
    const old = copy.interactions.get(row.interactionId);
    if (old) {
      const { status: _a, responseCommandId: _b, ...identity } = old;
      const { status: _c, responseCommandId: _d, ...nextIdentity } = row;
      if (!same(identity, nextIdentity)) return fail("stale_binding");
      if (old.status !== "pending" && !same(old, row))
        return fail("already_answered");
    } else {
      if (
        row.status !== "pending" ||
        source.dispatch.certainty !== "submitted" ||
        !["dispatching", "running"].includes(source.state)
      )
        return fail("invalid_input");
      const pending = batch.events.filter(
        (event) =>
          event.body.type === "interaction" &&
          event.body.interactionId === row.interactionId,
      );
      if (
        pending.length !== 1 ||
        pending[0].commandId !== row.commandId ||
        pending[0].generation !== row.generation ||
        pending[0].body.type !== "interaction" ||
        pending[0].body.status !== "pending" ||
        !same(
          "request" in pending[0].body ? pending[0].body.request : undefined,
          row.request,
        )
      )
        return fail("invalid_input");
    }
    if (
      [...copy.interactions.values()].some(
        (other) =>
          other.interactionId !== row.interactionId &&
          other.generation === row.generation &&
          other.nativeCallbackId === row.nativeCallbackId,
      )
    )
      return fail("content_conflict");
    if (
      old?.status === "pending" &&
      row.status === "expired" &&
      (!Number.isSafeInteger(batch.nowMs) || batch.nowMs! <= row.expiresAtMs)
    )
      return fail("invalid_input");
    if (row.status === "answered") {
      const response = copy.commands.get(row.responseCommandId!);
      if (
        !response ||
        response.command.input.type !== "respond" ||
        response.command.input.interactionId !== row.interactionId ||
        response.command.input.generation !== row.generation ||
        response.command.input.nativeRunId !== row.nativeRunId ||
        response.receipt.acceptedAtMs > row.expiresAtMs
      )
        return fail("invalid_input");
    }
    if (
      old &&
      old.status !== row.status &&
      !batch.events.some(
        (e) =>
          e.commandId === row.commandId &&
          e.body.type === "interaction" &&
          e.body.interactionId === row.interactionId &&
          e.body.status === row.status,
      )
    )
      return fail("invalid_input");
    copy.interactions.set(row.interactionId, clone(row));
  }
  for (const event of batch.events) {
    if (event.body.type !== "interaction") continue;
    const body = event.body;
    const row = copy.interactions.get(body.interactionId);
    if (
      !row ||
      row.commandId !== event.commandId ||
      row.generation !== event.generation ||
      row.status !== body.status
    )
      return fail("invalid_input");
    if (body.status === "pending") {
      if (
        state.interactions.has(row.interactionId) ||
        !interactionIds.has(row.interactionId) ||
        !same(body.request, row.request)
      )
        return fail("invalid_input");
    } else if ("request" in body) return fail("invalid_input");
  }
  const deliveryIds = new Set<Id>();
  for (const row of batch.deliveries) {
    if (deliveryIds.has(row.operationId)) return fail("invalid_input");
    deliveryIds.add(row.operationId);
    if (
      namespaceKey(row.namespace) !== namespaceKey(batch.namespace) ||
      !ids.has(row.eventId) ||
      row.contentHash !==
        deliveryFingerprint(
          copy.events.find((e) => e.eventId === row.eventId)!,
          row.target,
          limits,
        )
    )
      return fail("invalid_input");
    const old = copy.deliveries.get(row.operationId);
    if (
      old &&
      (old.contentHash !== row.contentHash ||
        old.target !== row.target ||
        old.eventId !== row.eventId ||
        old.retry !== row.retry ||
        row.attempts < old.attempts ||
        (old.status === "delivered" && row.status !== "delivered"))
    )
      return fail("content_conflict");
    copy.deliveries.set(row.operationId, clone(row));
  }
  const surfaceIds = new Set<Id>();
  for (const row of batch.surfaces) {
    const interaction = copy.interactions.get(row.interactionId);
    if (
      surfaceIds.has(row.surfaceInstanceId) ||
      namespaceKey(row.namespace) !== namespaceKey(batch.namespace) ||
      row.generation !== batch.expectedGeneration ||
      !interaction ||
      row.generation !== interaction.generation ||
      row.nativeRunId !== interaction.nativeRunId
    )
      return fail("invalid_input");
    surfaceIds.add(row.surfaceInstanceId);
    const old = copy.surfaces.get(row.surfaceInstanceId);
    if (old) {
      const { revision: _a, status: _b, ...identity } = old;
      const { revision: _c, status: _d, ...nextIdentity } = row;
      if (!same(identity, nextIdentity)) return fail("stale_binding");
      if (old.status !== "active" || row.revision !== old.revision + 1)
        return fail("revision_conflict", "same_command");
    } else if (
      row.status !== "active" ||
      row.revision !== 0 ||
      interaction.status !== "pending"
    )
      return fail("invalid_input");
    if (row.status === "invalidated") {
      const source = copy.commands.get(interaction.commandId);
      const invalidation = batch.events.filter(
        (e) =>
          e.body.type === "surface_invalidated" &&
          e.body.surfaceInstanceId === row.surfaceInstanceId,
      );
      if (
        !old ||
        !source ||
        !isSettled(source) ||
        invalidation.length !== 1 ||
        invalidation[0].body.type !== "surface_invalidated" ||
        invalidation[0].body.revision !== row.revision ||
        invalidation[0].commandId !== interaction.commandId
      )
        return fail("invalid_input");
      copy.surfaces.set(row.surfaceInstanceId, clone(row));
      continue;
    }
    const matching = batch.events.filter(
      (e) =>
        e.body.type === "surface" &&
        e.body.surfaceInstanceId === row.surfaceInstanceId,
    );
    const expected = !old
      ? "create"
      : row.status === "deleted"
        ? "delete"
        : "update";
    if (
      matching.length !== 1 ||
      matching[0].body.type !== "surface" ||
      matching[0].body.operation !== expected ||
      matching[0].body.revision !== row.revision ||
      matching[0].commandId !== interaction.commandId
    )
      return fail("invalid_input");
    const payload = matching[0].body.payload;
    const operationKeys =
      expected === "create"
        ? ["createSurface"]
        : expected === "delete"
          ? ["deleteSurface"]
          : ["updateComponents", "updateDataModel"];
    const keys = Object.keys(payload).filter((k) => k !== "version");
    if (
      payload.version !== "v0.9.1" ||
      keys.length !== 1 ||
      !operationKeys.includes(keys[0]) ||
      (payload[keys[0]] as { surfaceId?: unknown })?.surfaceId !== row.surfaceId
    )
      return fail("invalid_input");
    copy.surfaces.set(row.surfaceInstanceId, clone(row));
    if (row.status === "deleted" && interaction.status === "pending")
      return fail("invalid_input");
  }
  for (const e of batch.events)
    if (
      (e.body.type === "surface" || e.body.type === "surface_invalidated") &&
      !surfaceIds.has(e.body.surfaceInstanceId)
    )
      return fail("invalid_input");
  for (const record of batch.commands) {
    if (!isSettled(record)) continue;
    const commandId = record.command.commandId;
    if (
      [...copy.interactions.values()].some(
        (i) => i.commandId === commandId && i.status === "pending",
      ) ||
      [...copy.surfaces.values()].some(
        (surface) =>
          surface.status === "active" &&
          copy.interactions.get(surface.interactionId)?.commandId === commandId,
      )
    )
      return fail("invalid_input");
  }
  // A direct commit must satisfy the same surface fence as accept(), including
  // a deletion or revision change carried by this very batch.
  for (const interaction of batch.interactions) {
    if (interaction.status !== "answered") continue;
    const response = copy.commands.get(interaction.responseCommandId!)!.command
      .input;
    if (response.type !== "respond") return fail("invalid_input");
    const linked = [...copy.surfaces.values()].filter(
      (row) => row.interactionId === interaction.interactionId,
    );
    if (linked.length || response.surface) {
      const surface =
        response.surface && copy.surfaces.get(response.surface.instanceId);
      if (
        !surface ||
        surface.status !== "active" ||
        surface.interactionId !== interaction.interactionId ||
        surface.revision !== response.surface!.revision
      )
        return fail("stale_binding");
    }
  }
  copy.session = clone(batch.session);
  return ok(copy);
}

/** Verified handoff changes observation authority, never the original attempt identity. */
function reduceRebind(
  state: SessionState,
  input: SessionRebind,
  limits: Limits = defaultLimits,
): Result<SessionState> {
  if (!isId(input.eventId)) return fail("invalid_input");
  const checked = checkState(
    state,
    input.expectedRevision,
    input.expectedGeneration,
  );
  if (!checked.ok) return checked;
  if (namespaceKey(input.namespace) !== namespaceKey(state.session.namespace))
    return fail("permission_denied");
  if (
    !(input.restored instanceof VerifiedProviderSession) ||
    !VerifiedProviderSession.prototype.restores.call(
      input.restored,
      state.session,
    )
  )
    return fail("permission_denied");
  const binding = input.restored.binding,
    capabilities = input.restored.capabilities;
  if (
    state.generations.has(binding.generation) ||
    state.session.capabilities.continuation !== "across_processes"
  )
    return fail("stale_binding");
  const copy = clone(state),
    events: Event[] = [];
  const append = (
    fields: { commandId?: Id; attemptId?: Id; body: Event["body"] },
    label: string,
  ) => {
    const event = {
      schemaVersion: 2,
      kind: "event",
      namespace: input.namespace,
      eventId: eventId(input.eventId, label),
      sequence: state.session.lastSequence + events.length + 1,
      generation: binding.generation,
      ...fields,
    } as Event;
    valid(event, limits);
    if (state.events.some((e) => e.eventId === event.eventId))
      throw new ContractError("context");
    events.push(event);
  };
  append(
    {
      body: {
        type: "session_rebound",
        previousGeneration: state.session.binding.generation,
      },
    },
    "rebind",
  );
  for (const [id, c] of copy.commands) {
    if (isSettled(c)) continue;
    if (c.dispatch) {
      const dispatch = {
        ...c.dispatch,
        observerGeneration: binding.generation,
      };
      copy.commands.set(id, {
        schemaVersion: 2,
        kind: "commandRecord",
        command: c.command,
        receipt: c.receipt,
        state: "reconciliation_required",
        dispatch,
      });
      append(
        {
          commandId: id,
          attemptId: dispatch.attemptId,
          body: { type: "status", state: "reconciliation_required" },
        },
        `command-${id}`,
      );
    } else if (
      c.command.input.type !== "prompt" ||
      c.command.input.policy !== "queue_next"
    ) {
      const failure = {
        code: "stale_binding" as const,
        retry: "never" as const,
      };
      copy.commands.set(id, { ...c, state: "invalidated", failure });
      append(
        { commandId: id, body: { type: "invalidated", failure } },
        `command-${id}`,
      );
    }
  }
  for (const [id, row] of copy.interactions) {
    if (row.status !== "pending") continue;
    copy.interactions.set(id, { ...row, status: "unavailable" });
    append(
      {
        commandId: row.commandId,
        attemptId: copy.commands.get(row.commandId)!.dispatch!.attemptId,
        body: { type: "interaction", interactionId: id, status: "unavailable" },
      },
      `interaction-${id}`,
    );
  }
  for (const [id, row] of copy.surfaces) {
    if (row.status !== "active") continue;
    const next = {
      ...row,
      status: "invalidated" as const,
      revision: row.revision + 1,
    };
    valid(next, limits);
    copy.surfaces.set(id, next);
    const interaction = copy.interactions.get(row.interactionId)!;
    append(
      {
        commandId: interaction.commandId,
        attemptId: copy.commands.get(interaction.commandId)!.dispatch!
          .attemptId,
        body: {
          type: "surface_invalidated",
          surfaceInstanceId: id,
          revision: next.revision,
        },
      },
      `surface-${id}`,
    );
  }
  copy.events.push(...events);
  copy.session = {
    ...copy.session,
    binding,
    capabilities,
    revision: copy.session.revision + 1,
    lastSequence: copy.session.lastSequence + events.length,
  };
  valid(copy.session, limits);
  copy.generations.add(binding.generation);
  return ok(copy);
}

function reduceRetirement(
  state: SessionState,
  revision: Counter,
  generation: Id,
  limits: Limits = defaultLimits,
): Result<SessionState> {
  const checked = checkState(state, revision, generation);
  if (!checked.ok) return checked;
  if (
    [...state.commands.values()].some((c) => !isSettled(c)) ||
    [...state.interactions.values()].some((i) => i.status === "pending") ||
    [...state.deliveries.values()].some((d) => d.status !== "delivered")
  )
    return fail("reconciliation_required", "reconcile_first");
  const copy = clone(state);
  copy.session = {
    ...copy.session,
    status: "retired",
    revision: revision + 1,
    lastSequence: copy.session.lastSequence + 1,
  };
  const event: Event = {
    schemaVersion: 2,
    kind: "event",
    namespace: copy.session.namespace,
    eventId: eventId(
      namespaceKey(copy.session.namespace),
      `retire-${revision}`,
    ),
    sequence: copy.session.lastSequence,
    generation,
    body: { type: "session_retired" },
  };
  valid(copy.session, limits);
  valid(event, limits);
  copy.events.push(event);
  return ok(copy);
}
export function canPrune(state: SessionState, nowMs: Counter): boolean {
  return (
    Number.isSafeInteger(nowMs) &&
    nowMs >= 0 &&
    state.session.status === "retired" &&
    [...state.commands.values()].every(
      (c) => c.receipt.receiptUntilMs < nowMs,
    ) &&
    [...state.deliveries.values()].every((d) => d.status === "delivered")
  );
}

function guarded<T>(action: () => Result<T>): Result<T> {
  try {
    return action();
  } catch (error) {
    return fail(
      error instanceof ContractError && error.code === "limit"
        ? "limit_exceeded"
        : "invalid_input",
    );
  }
}
export function acceptCommand(
  state: SessionState,
  input: AcceptCommand,
  limits: Limits = defaultLimits,
): Result<{ state: SessionState; receipt: import("./wire.js").Receipt }> {
  return guarded(() => reduceAcceptance(state, input, limits));
}
export function rebindSession(
  state: SessionState,
  input: SessionRebind,
  limits: Limits = defaultLimits,
): Result<SessionState> {
  return guarded(() => reduceRebind(state, input, limits));
}
export function retireSession(
  state: SessionState,
  revision: Counter,
  generation: Id,
  limits: Limits = defaultLimits,
): Result<SessionState> {
  return guarded(() => reduceRetirement(state, revision, generation, limits));
}
function dispatchDeadline(state: SessionState, c: CommandRecord): number {
  return state.events.some(
    (e) =>
      e.commandId === c.command.commandId &&
      e.body.type === "reconciled" &&
      e.body.resolution === "not_submitted",
  )
    ? Math.min(c.command.expiresAtMs, c.receipt.retryUntilMs)
    : c.command.expiresAtMs;
}
