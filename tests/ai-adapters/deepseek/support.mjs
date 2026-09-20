import { DeepSeekAdapter } from "../../../packages/ai-adapters/deepseek/dist/adapter.js";
import {
  COMPOSITION_ID,
  ACTIVE_PROFILE_ID,
} from "../../../packages/ai-adapters/deepseek/dist/assembly.js";
import { fixtureSession } from "../../../packages/ai-contract/dist/testing/index.js";
export const configuration = (
  directory = process.cwd(),
  permissions = "tools_disabled",
) => ({
  namespace: fixtureSession().namespace,
  provider: "deepseek",
  config: { id: "config-1", revision: "1" },

  workingDirectory: directory,
  permissions,
});
export const budget = (timeoutMs = 5000) => ({
  timeoutMs,
  signal: new AbortController().signal,
});
export function scriptedAdapter(scenario = "submitted", overrides = {}) {
  const config = configuration(),
    events = [];
  let notify = () => {},
    stop;
  const runtime = {
    stopped: new Promise((r) => (stop = r)),
    onEvent(fn) {
      notify = fn;
    },
    stop() {
      stop();
    },
    async call(op, value) {
      if (op === "initialize")
        return {
          composition: COMPOSITION_ID,
          nativeSessionId: value.nativeSessionId,
          activation: ACTIVE_PROFILE_ID,
          observationOnly: value.restore,
        };
      if (op === "prompt") {
        if (scenario === "unknown") throw Error("transport lost");
        return {
          status: "accepted",
          requestId: value.requestId,
          activation: ACTIVE_PROFILE_ID,
          observationOnly: false,
        };
      }
      if (op === "inspect") return { status: "unknown" };
      if (op === "cancel") return {};
      return {};
    },
    ...overrides,
  };
  const port = new DeepSeekAdapter(
    {
      clock: { now: () => 0 },
      resolveConfiguration: async () => ({
        configuration: config,
        persistenceDirectory: "/tmp/rss-deepseek-fixture",
        apiUrl: "https://custom.example.test/v1",
        apiKey: "fixture",
        model: "deepseek-chat",
      }),
    },
    () => runtime,
  );
  // Conformance fixture has a finite observation stream and no terminal fact.
  const observe = port.observe.bind(port);
  port.observe = (binding, b) => observe(binding, { ...b, timeoutMs: 5 });
  return port;
}
