import { afterEach, expect, it, vi } from "vitest";
import { createSystemSettings, type SettingsPort } from "./controller";
import { fixtureLocalPorts } from "../../../../tests/assistant/local-ports";
import type {
  SettingsOperation,
  SettingsResponse,
  SettingsSnapshot,
} from "../self-service/types";
const controllers: ReturnType<typeof createSystemSettings>[] = [];
it("retains a lost collection UUID and queries it before another refresh, while ordinary reads never submit", async () => {
  const data = fixtureLocalPorts().data!;
  data.query_submit = vi.fn(async () => {
    throw Error("lost");
  });
  data.query_status = vi.fn(async ({ request }) => ({
    request,
    query: { id: "hardware.network", revision: "1".repeat(64) },
    phase: "completed" as const,
    cancelRequested: false,
    preparation: null,
  }));
  let counter = 0;
  const controller = createSystemSettings(
    async (operation) =>
      response({ request: "request" in operation ? operation.request : null }),
    data,
    () => `request-${++counter}`,
  );
  controllers.push(controller);
  await controller.load("network");
  expect(data.query_submit).not.toHaveBeenCalled();
  await controller.load("network", true);
  const original = [...controller.state.queries.values()][0].request;
  expect(data.query_submit).toHaveBeenCalledTimes(1);
  await controller.load("network", true);
  expect(data.query_submit).toHaveBeenCalledTimes(1);
  expect(data.query_status).toHaveBeenCalledWith({ request: original });
});
afterEach(() => {
  for (const controller of controllers.splice(0)) controller.dispose();
  vi.useRealTimers();
});
const snapshot: SettingsSnapshot = {
  section: "display",
  revision: "a".repeat(64),
  collectedAtUnixMs: 1000,
  source: "native fixture",
  targets: [],
};
const response = (
  values: Partial<SettingsResponse> = {},
): SettingsResponse => ({
  writeAllowed: false,
  request: null,
  digest: null,
  finished: true,
  effect: "observed",
  snapshot,
  keepBeforeUnixMs: null,
  reason: null,
  ...values,
});
function setup(port: SettingsPort) {
  const identity = vi.fn(() => "00000000-0000-4000-8000-000000000001");
  const controller = createSystemSettings(port, null, identity);
  controllers.push(controller);
  return { controller, identity };
}
it("keeps an original UUID after response loss; retained reads cannot discard it or resubmit a mutation", async () => {
  const calls: SettingsOperation[] = [];
  const { controller, identity } = setup(async (operation) => {
    calls.push(operation);
    if (operation.kind === "change") throw Error("response lost");
    return response();
  });
  await controller.load("display");
  const patch = {
    kind: "display" as const,
    target: "display",
    mode: "mode",
    origin: null,
  };
  await controller.save(patch, snapshot.revision);
  expect(controller.state.pending.get("display")?.request).toBe(
    "00000000-0000-4000-8000-000000000001",
  );
  await controller.load("display");
  expect(controller.state.pending.has("display")).toBe(true);
  await controller.recover("display");
  expect(calls.filter((call) => call.kind === "change")).toHaveLength(1);
  expect(calls.at(-1)).toEqual({
    kind: "status",
    request: "00000000-0000-4000-8000-000000000001",
  });
  expect(identity).toHaveBeenCalledTimes(1);
  expect(controller.state.pending.get("display")?.operation.kind).toBe(
    "change",
  );
});
it("preview keep uses the exact frozen digest and does not allocate another operation", async () => {
  vi.useFakeTimers();
  const calls: SettingsOperation[] = [];
  const { controller, identity } = setup(async (operation) => {
    calls.push(operation);
    return response({
      request: "00000000-0000-4000-8000-000000000001",
      digest: "b".repeat(64),
      finished: false,
      effect: "preview",
      keepBeforeUnixMs: Date.now() + 15000,
    });
  });
  const refresh = controller.load("display", true);
  await vi.advanceTimersByTimeAsync(1);
  expect(controller.state.observations.get("display")?.effect).toBe("preview");
  await controller.keep("display");
  expect(calls.find((c) => c.kind === "keepDisplay")).toEqual({
    kind: "keepDisplay",
    request: "00000000-0000-4000-8000-000000000001",
    digest: "b".repeat(64),
  });
  expect(identity).toHaveBeenCalledTimes(1);
  controller.dispose();
  await vi.advanceTimersByTimeAsync(250);
  await refresh;
  expect(
    calls.every((c) =>
      ["refresh", "status", "keepDisplay", "snapshot"].includes(c.kind),
    ),
  ).toBe(true);
});
it("late native responses cannot publish into a retired user's page", async () => {
  let complete!: (value: SettingsResponse) => void;
  const { controller } = setup(
    () =>
      new Promise((resolve) => {
        complete = resolve;
      }),
  );
  const pending = controller.load("display");
  controller.dispose();
  complete(response());
  await pending;
  expect(controller.state.observations.size).toBe(0);
  expect(controller.state.facts.size).toBe(0);
});

it("missing status preserves the original write UUID; native settings remain independently accessible", async () => {
  const calls: SettingsOperation[] = [];
  const { controller, identity } = setup(async (operation) => {
    calls.push(operation);
    if (operation.kind === "change") throw Error("response lost");
    if (operation.kind === "status")
      throw Object.assign(Error("missing"), { code: "notFound" });
    if (operation.kind === "snapshot") return response();
    return response({
      request: "00000000-0000-4000-8000-000000000002",
      effect: "openRequested",
    });
  });
  identity
    .mockReturnValueOnce("00000000-0000-4000-8000-000000000001")
    .mockReturnValueOnce("00000000-0000-4000-8000-000000000002");
  await controller.load("display");
  await controller.save(
    { kind: "display", target: "display", mode: "mode", origin: null },
    snapshot.revision,
  );
  await controller.recover("display");
  expect(controller.state.pending.get("display")?.request).toBe(
    "00000000-0000-4000-8000-000000000001",
  );
  await controller.openNative("display");
  expect(calls.filter((c) => c.kind === "change")).toHaveLength(1);
  expect(calls.find((c) => c.kind === "openNative")).toEqual({
    kind: "openNative",
    section: "display",
    request: "00000000-0000-4000-8000-000000000002",
  });
  expect(controller.state.pending.get("display")?.request).toBe(
    "00000000-0000-4000-8000-000000000001",
  );
});
