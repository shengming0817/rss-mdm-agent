import assert from "node:assert/strict";
import type {
  AcceptCommand,
  Caller,
  SessionStore,
  SessionCommit,
  Result,
} from "../ports.js";
import type { Command, Session } from "../wire.js";
export const fixtureCaller: Caller = {
  tenantId: "tenant-1",
  principalId: "user-1",
  authorityId: "authority-1",
};
export function fixtureSession(): Session {
  return {
    schemaVersion: 2,
    kind: "session",
    namespace: { ...fixtureCaller, sessionId: "session-1" },
    revision: 0,
    lastSequence: 0,
    status: "active",
    binding: {
      provider: "fake",
      providerVersion: "fixture-1",
      adapterVersion: "fixture-1",
      generation: "generation-1",
      accountRef: "account-1",
      nativeSessionId: "native-1",
      config: { id: "config-1", revision: "1" },
    },
    capabilities: {
      continuation: "unsupported",
      cancellation: "request_only",
      tools: "disabled",
      steer: "unsupported",
      fork: "unsupported",
      subagent: "unsupported",
      terminal: "unsupported",
      structuredQuestion: "unsupported",
      multimodal: "unsupported",
    },
  };
}
export function fixtureCommand(id = "command-1"): Command {
  return {
    schemaVersion: 2,
    kind: "command",
    sessionId: "session-1",
    commandId: id,
    expiresAtMs: 1000,
    input: { type: "prompt", text: "fixture text", policy: "queue_next" },
  };
}
export function acceptance(
  session = fixtureSession(),
  command = fixtureCommand(),
): AcceptCommand {
  return {
    namespace: session.namespace,
    command,
    expectedRevision: session.revision,
    expectedGeneration: session.binding.generation,
    nowMs: 0,
    retention: { retryWindowMs: 100, receiptWindowMs: 200 },
    event: {
      schemaVersion: 2,
      kind: "event",
      namespace: session.namespace,
      eventId: `event-${command.commandId}`,
      sequence: session.lastSequence + 1,
      commandId: command.commandId,
      generation: session.binding.generation,
      body: { type: "status", state: "accepted" },
    },
  };
}
export function unwrap<T>(result: Result<T>): T {
  assert.equal(result.ok, true, JSON.stringify(result));
  if (!result.ok) throw new Error("conformance failed");
  return result.value;
}
export function emptyCommit(session: Session): SessionCommit {
  return {
    namespace: session.namespace,
    expectedRevision: session.revision,
    expectedGeneration: session.binding.generation,
    session: { ...session, revision: session.revision + 1 },
    commands: [],
    events: [],
    interactions: [],
    deliveries: [],
    surfaces: [],
  };
}
/** Shared behavior suite; each adapter supplies an isolated store. Real backend durability
 * and process faults must be supplied separately by the adapter's own harness. */
export async function runStoreConformance(
  create: () => Promise<SessionStore> | SessionStore,
): Promise<void> {
  const store = await create(),
    s = fixtureSession(),
    input = acceptance(s);
  unwrap(await store.create(s));
  const [a, b] = await Promise.all([store.accept(input), store.accept(input)]);
  assert.deepEqual(unwrap(a), unwrap(b));
  assert.equal(
    unwrap(await store.snapshot(s.namespace, 1024)).commands.length,
    1,
  );
  const conflict = await store.accept({
    ...input,
    command: {
      ...input.command,
      input: { type: "prompt", text: "different", policy: "queue_next" },
    },
  });
  assert.deepEqual(conflict, {
    ok: false,
    error: { code: "content_conflict", retry: "never" },
  });
  assert.deepEqual(await store.accept({ ...input, nowMs: 201 }), {
    ok: false,
    error: { code: "expired", retry: "never" },
  });
  assert.deepEqual(
    await store.command(
      { ...s.namespace, tenantId: "other" },
      input.command.commandId,
    ),
    { ok: false, error: { code: "session_gone", retry: "never" } },
  );
  const current = unwrap(await store.session(s.namespace));
  assert.deepEqual(
    await store.commit({
      ...emptyCommit(current),
      expectedGeneration: "stale",
    }),
    { ok: false, error: { code: "stale_binding", retry: "never" } },
  );
  assert.deepEqual(await store.commit(emptyCommit(s)), {
    ok: false,
    error: { code: "revision_conflict", retry: "same_command" },
  });
  const accepted = unwrap(
    await store.command(s.namespace, input.command.commandId),
  );
  const dispatch = {
    generation: s.binding.generation,
    nativeSessionId: s.binding.nativeSessionId,
    certainty: "unknown" as const,
  };
  unwrap(
    await store.commit({
      ...emptyCommit(current),
      commands: [{ ...accepted, state: "dispatching", dispatch }],
    }),
  );
  const dispatching = unwrap(await store.session(s.namespace));
  unwrap(
    await store.commit({
      ...emptyCommit(dispatching),
      commands: [{ ...accepted, state: "reconciliation_required", dispatch }],
    }),
  );
  const unknown = unwrap(await store.session(s.namespace));
  assert.equal(
    (await store.recovery(10)).items[0].state,
    "reconciliation_required",
  );
  assert.equal(
    (await store.commit({ ...emptyCommit(unknown), commands: [accepted] })).ok,
    false,
    "unknown submission cannot return to accepted",
  );
  unwrap(
    await store.commit({
      ...emptyCommit(unknown),
      commands: [{ ...accepted, state: "terminal", outcome: "completed" }],
    }),
  );
  const terminal = unwrap(await store.session(s.namespace));
  unwrap(
    await store.retire(s.namespace, terminal.revision, s.binding.generation),
  );
  assert.equal(await store.pruneRetired(200), 0);
  assert.equal(await store.pruneRetired(201), 1);
  assert.equal((await store.accept(input)).ok, false);
  assert.equal(
    (await store.create(s)).ok,
    false,
    "retired namespace cannot be resurrected",
  );
}
