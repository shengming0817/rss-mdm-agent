import { test } from "node:test";
import assert from "node:assert/strict";
import { createFixture, startFixture } from "./server.mjs";

test("one fixture simulates exact confirmation, replay and cancellation without producing a terminal effect", async (t) => {
  const f = await createFixture();
  t.after(() => f.close());
  f.selectScenario("proposed");
  const offer = f.snapshot().available[0];
  assert.equal(f.details(offer.request).value.state, "awaitingConfirmation");
  assert.throws(
    () => f.execute({ ...offer, revision: "b".repeat(64) }),
    /mismatch/,
  );
  assert.equal(f.details(offer.request).value.state, "awaitingConfirmation");
  assert.equal(f.execute(offer).confirmationRequired, true);
  assert.equal(f.details(offer.request).value.state, "awaitingConfirmation");
  assert.throws(() => f.confirm({ ...offer, attempt: "other" }), /mismatch/);
  assert.equal(f.confirm(offer).request, offer.request);
  assert.equal(f.confirm(offer).request, offer.request);
  assert.equal(f.snapshot().requests.length, 1);
  assert.equal(
    f.cancel({ requestId: offer.request }).value.status.cancelRequested,
    true,
  );
  assert.equal(
    f.cancel({ requestId: offer.request }).value.status.cancelRequested,
    true,
  );
  f.selectScenario("outcomeUnknown");
  assert.equal(f.details(offer.request).value.status.phase, "outcomeUnknown");
  assert.equal(f.execute(offer).request, offer.request);
  assert.equal(f.details(offer.request).value.status.phase, "outcomeUnknown");
});
test("diagnostic and denied scenarios cannot be mistaken for a successful execution", async (t) => {
  const f = await createFixture();
  t.after(() => f.close());
  for (const scenario of [
    "notInstalled",
    "disconnected",
    "mismatch",
    "registrationRequired",
    "notReady",
    "serviceRejected",
    "configurationRequired",
  ]) {
    f.selectScenario(scenario);
    assert.throws(() => f.snapshot(), /unavailable/);
  }
  f.selectScenario("denied");
  const offer = f.snapshot().available[0];
  assert.throws(() => f.execute(offer), /denied/);
  assert.equal(f.details(offer.request).value.state, "failed");
});
test("browser factory and native Vite plugin share the same scenario and RPC owner", async (t) => {
  const f = await startFixture();
  t.after(() => f.close());
  const origin = new URL(f.url).origin;
  await fetch(origin + "/__fixture/scenario", {
    method: "POST",
    body: JSON.stringify({ scenario: "registrationRequired" }),
  });
  const state = await (await fetch(origin + "/__fixture/state")).json();
  assert.deepEqual(state.service, f.serviceView());
  assert.equal(state.service.status.readiness.phase, "registrationRequired");
  assert.equal((await fetch(origin + "/__fixture/snapshot")).status, 500);
});

test("fixture supplies isolated current identity and Host operations with generation fencing", async (t) => {
  const f = await startFixture();
  t.after(() => f.close());
  const origin = new URL(f.url).origin;
  const read = async (method, input) => {
    const r = await fetch(
      origin + "/__fixture/" + method,
      input === undefined
        ? undefined
        : { method: "POST", body: JSON.stringify(input) },
    );
    return { status: r.status, value: await r.json() };
  };
  const identity = (await read("identity")).value;
  assert.match(identity.user.displayName, /fixture/);
  const host = (await read("host")).value;
  assert.equal(host.phase, "ready");
  assert.match(host.version, /fixture/);
  const restarted = await read("host-restart", { generation: host.generation });
  assert.equal(restarted.value.generation, host.generation + 1);
  assert.equal(
    (await read("host-restart", { generation: host.generation })).status,
    500,
  );
  assert.equal((await read("host-export")).value, false);
});
