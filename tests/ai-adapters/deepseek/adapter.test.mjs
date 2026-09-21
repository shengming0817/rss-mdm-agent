import test from "node:test";
import {
  runProviderConformance,
  fixtureSession,
} from "../../../packages/ai-contract/dist/testing/index.js";
import { createDeepSeekAdapter } from "../../../packages/ai-adapters/deepseek/dist/index.js";
import { scriptedAdapter } from "./support.mjs";

test("A01 provider conformance: submitted, unknown and late admission", async () => {
  await runProviderConformance(
    scriptedAdapter,
    {
      namespace: fixtureSession().namespace,
      provider: "deepseek",
      config: { id: "config-1", revision: "1" },

      workingDirectory: process.cwd(),
      permissions: "tools_disabled",
    },
    () => ({ timeoutMs: 2000, signal: new AbortController().signal }),
  );
});
test("public adapter constructor is available", () => {
  if (typeof createDeepSeekAdapter !== "function")
    throw Error("missing adapter");
});
