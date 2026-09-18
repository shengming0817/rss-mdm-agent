import assert from "node:assert/strict";
import { test } from "node:test";
import {
  ScriptedProvider,
  runProviderConformance,
  fixtureCommand,
  unwrap,
} from "../../packages/ai-contract/dist/testing/index.js";
const configuration = {
  config: { id: "config-1", revision: "1" },
  accountRef: "account-1",
  workingDirectory: ".",
  permissions: "tools_disabled",
};
const budget = () => ({ timeoutMs: 1000, signal: AbortSignal.timeout(1000) });
test("shared provider lifecycle conformance runs without Tauri", () =>
  runProviderConformance(new ScriptedProvider(), configuration, budget()));
test("unknown submit and cancel request do not manufacture terminal or tool authority", async () => {
  const port = new ScriptedProvider();
  unwrap(await port.initialize(configuration, budget()));
  const binding = unwrap(await port.createSession(budget()));
  port.submission = "unknown";
  assert.equal(
    (await port.submit(binding, fixtureCommand(), budget())).certainty,
    "unknown",
  );
  assert.equal(
    (await port.cancel(binding, fixtureCommand(), budget())).value,
    "request_only",
  );
  assert.equal(
    (await Array.fromAsync(port.observe(binding, budget()))).length,
    0,
  );
  assert.equal(
    (
      await port.initialize(
        {
          ...configuration,
          permissions: "host_mediated",
          tools: {
            propose: () => {
              throw new Error("must never execute");
            },
          },
        },
        budget(),
      )
    ).ok,
    false,
  );
});
