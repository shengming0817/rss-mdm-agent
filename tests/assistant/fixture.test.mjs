import { test } from "node:test";
import assert from "node:assert/strict";
import { createFixture, startFixture } from "./server.mjs";

test("one fixture simulates exact confirmation, replay and cancellation without producing a terminal effect", async (t) => {
  const f = await createFixture();
  t.after(() => f.close());
  f.selectScenario("proposed");
  const offer = f.snapshot().available[0];
  assert.equal(f.details(offer.request).value.state, "proposed");
  assert.throws(
    () => f.execute({ ...offer, revision: "b".repeat(64) }),
    /mismatch/,
  );
  assert.equal(f.details(offer.request).value.state, "proposed");
  assert.equal(f.execute(offer).request, offer.request);
  assert.equal(f.execute(offer).request, offer.request);
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
