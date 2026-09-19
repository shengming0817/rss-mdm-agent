import assert from "node:assert/strict";
import {
  withinBudget,
  boundedPort,
  withCleanup,
  closeAll,
  defaultBudget,
  type BudgetFactory,
} from "./budget.js";
import type { HostPort, SessionOptions } from "../ports.js";
import { fixtureCaller, fixtureCommand, unwrap } from "./conformance.js";
const options: SessionOptions = {
  provider: "fake",
  config: { id: "cfg", revision: "1" },
  accountRef: "account-1",
  profile: "conversation",
};
const budget = () => ({ timeoutMs: 1000, signal: AbortSignal.timeout(1000) });
/** Each factory supplies an isolated Host backed by a deterministic, non-terminal
 * provider and clock at zero. The same boundary suite applies to real Host coordinators. */
export async function runHostConformance(
  create: () => HostPort | Promise<HostPort>,
  factory: BudgetFactory = defaultBudget,
): Promise<void> {
  const host = await withinBudget(factory, create);
  await withCleanup(
    () => runHostScenarios(boundedPort(host, factory)),
    () => closeAll([host], factory),
  );
}
async function runHostScenarios(host: HostPort): Promise<void> {
  unwrap(
    host.negotiate({
      contractVersion: 2,
      acp: 1,
      durableReceipts: true,
      cursorAttach: true,
    }),
  );
  assert.equal(
    (
      await host.createSession(
        fixtureCaller,
        { ...options, profile: "controlled_tools" },
        budget(),
      )
    ).ok,
    false,
  );
  const session = unwrap(
    await host.createSession(fixtureCaller, options, budget()),
  );
  const command = {
    ...fixtureCommand(),
    sessionId: session.namespace.sessionId,
  };
  const receipt = unwrap(await host.submit(fixtureCaller, command, budget()));
  assert.deepEqual(
    unwrap(await host.submit(fixtureCaller, command, budget())),
    receipt,
  );
  const conflict = await host.submit(
    fixtureCaller,
    { ...command, expiresAtMs: 999 },
    budget(),
  );
  assert.equal(conflict.ok, false);
  if (!conflict.ok) assert.equal(conflict.error.code, "content_conflict");
  for (const caller of [
    { ...fixtureCaller, tenantId: "other" },
    { ...fixtureCaller, principalId: "other" },
    { ...fixtureCaller, authorityId: "other" },
  ]) {
    assert.equal(
      (
        await host.snapshotPage(
          caller,
          command.sessionId,
          { limit: 256 },
          budget(),
        )
      ).ok,
      false,
    );
    assert.equal((await host.submit(caller, command, budget())).ok, false);
  }
  const snapshot = unwrap(
    await host.snapshotPage(
      fixtureCaller,
      command.sessionId,
      { limit: 256 },
      budget(),
    ),
  );
  assert.equal(snapshot.cursor, 1);
  assert.equal(snapshot.events.at(-1)!.sequence, snapshot.cursor);
  const control = new AbortController();
  const stream = host
    .subscribe(fixtureCaller, command.sessionId, 0, {
      timeoutMs: 1000,
      signal: control.signal,
    })
    [Symbol.asyncIterator]();
  try {
    const replay = await stream.next();
    assert.equal(replay.value?.type, "event");
    if (replay.value?.type === "event")
      assert.equal(replay.value.event.commandId, command.commandId);
    const pending = stream.next();
    unwrap(
      await host.submit(
        fixtureCaller,
        { ...command, commandId: "command-2" },
        budget(),
      ),
    );
    const live = await pending;
    assert.equal(live.value?.type, "event");
    if (live.value?.type === "event")
      assert.equal(live.value.event.commandId, "command-2");
  } finally {
    control.abort();
    await stream.return?.();
  }
  assert.equal(
    unwrap(
      await host.snapshotPage(
        fixtureCaller,
        command.sessionId,
        { limit: 256 },
        budget(),
      ),
    ).commands[0].state,
    "accepted",
  );
  const expired = host
    .subscribe(fixtureCaller, command.sessionId, 999, budget())
    [Symbol.asyncIterator]();
  try {
    assert.equal((await expired.next()).value?.type, "resync_required");
  } finally {
    await expired.return?.();
  }
  const cancellation = {
    ...command,
    commandId: "cancel-1",
    input: {
      type: "cancel" as const,
      targetCommandId: command.commandId,
      generation: "stale",
    },
  };
  const stale = await host.cancel(fixtureCaller, cancellation, budget());
  assert.equal(stale.ok, false);
  if (!stale.ok) assert.equal(stale.error.code, "stale_binding");
  const answer = await host.respond(
    fixtureCaller,
    {
      ...command,
      commandId: "answer-1",
      input: {
        type: "respond",
        interactionId: "absent",
        generation: "stale",
        answer: { choice: "allow" },
      },
    },
    budget(),
  );
  assert.equal(answer.ok, false);
  if (!answer.ok) assert.equal(answer.error.code, "stale_binding");
  unwrap(
    await host.cancel(
      fixtureCaller,
      {
        ...cancellation,
        input: {
          ...cancellation.input,
          generation: session.binding.generation,
        },
      },
      budget(),
    ),
  );
  assert.equal(
    unwrap(
      await host.snapshotPage(
        fixtureCaller,
        command.sessionId,
        { limit: 256 },
        budget(),
      ),
    ).commands[0].state,
    "accepted",
  );
  const head = unwrap(
    await host.snapshotPage(
      fixtureCaller,
      command.sessionId,
      { limit: 256 },
      budget(),
    ),
  );
  const waiting = host
    .subscribe(fixtureCaller, command.sessionId, head.cursor, budget())
    [Symbol.asyncIterator]();
  const pending = waiting.next();
  unwrap(await host.close(budget()));
  assert.equal(
    (await pending).done,
    true,
    "close terminates pending subscriptions",
  );
  assert.equal(
    (await host.createSession(fixtureCaller, options, budget())).ok,
    false,
  );
  assert.equal((await host.submit(fixtureCaller, command, budget())).ok, false);
  assert.equal(
    (
      await host.snapshotPage(
        fixtureCaller,
        command.sessionId,
        { limit: 256 },
        budget(),
      )
    ).ok,
    false,
  );
  unwrap(await host.close(budget()));
}
