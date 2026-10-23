import { expect, it, vi } from "vitest";
import { createSoftware } from "./controller";
import type {
  DataAccessPort,
  InstalledSoftware,
  LocalSoftwareStatus,
  QueryDescriptor,
  SoftwarePage,
  SoftwarePort,
} from "../self-service/types";
const query = { id: "native:software", revision: "1".repeat(64) };
const descriptor: QueryDescriptor = {
  reference: query,
  available: true,
  domains: ["software.installations"],
  inputSchema: {},
  kind: "native",
  policy: null,
  reason: null,
  scope: { kind: "device" },
  title: "Installed software",
};
const item: InstalledSoftware = {
  reference: {
    query,
    cursor: { snapshot: "2".repeat(64), offset: 0 },
    installation: "app",
  },
  kind: "application",
  name: "Example",
  version: "1.0.0",
  nativeId: "org.example.app",
  origin: "application",
  scope: { kind: "device" },
  publisher: "TEAM",
  actions: [
    { action: "uninstall", available: true, reason: null, targetVersion: null },
    {
      action: "update",
      available: false,
      reason: "updateUnknown",
      targetVersion: null,
    },
  ],
};
const page: SoftwarePage = {
  query,
  collectedAt: 1000,
  quality: "complete",
  enumerationComplete: true,
  stale: false,
  items: [item],
  next: null,
};
const original = (request: string): LocalSoftwareStatus => ({
  detected: null,
  prepared: {
    request,
    digest: "3".repeat(64),
    choice: { request, software: item.reference, action: "uninstall" },
    name: item.name,
    identity: "system",
    initiator: {
      kind: "human",
      osSession: {
        device: "device",
        account: { platform: "macos", subject: "501" },
        session: "login/1",
      },
    },
    targetVersion: null,
    startBeforeUnixMs: Date.now() + 60_000,
  },
  execution: {
    operationRequestId: request,
    contentDigest: "3".repeat(64),
    phase: "waiting",
    mode: "test",
    process: null,
    software: null,
    attemptId: null,
    attempts: 0,
    cancelRequested: false,
    admission: null,
    dispatchCause: null,
    stopOutcome: null,
    assessment: null,
    evidence: [],
  },
});
function ports() {
  const data: DataAccessPort = {
    query_directory: vi.fn(async () => ({
      items: [descriptor],
      next: null,
      policyNext: null,
    })),
    data_read: vi.fn(),
    query_submit: vi.fn(),
    query_status: vi.fn(),
    query_cancel: vi.fn(),
  };
  const port: SoftwarePort = {
    read: vi.fn(async () => structuredClone(page)),
    history: vi.fn(async () => ({ items: [], next: null })),
    prepare: vi.fn(async (c) => original(c.request).prepared),
    submit: vi.fn(async (c) => original(c.request)),
    status: vi.fn(async (c) => original(c.request)),
    cancel: vi.fn(async (c) => ({
      ...original(c.request),
      execution: { ...original(c.request).execution, cancelRequested: true },
    })),
  };
  return { data, port };
}
it("reads retained facts without collecting and never counts an unknown candidate as an update", async () => {
  const { data, port } = ports();
  const c = createSoftware(data, port);
  await c.discover();
  expect(c.summary.value).toMatchObject({
    installed: 1,
    updates: 0,
    complete: true,
  });
  expect(data.query_submit).not.toHaveBeenCalled();
  expect(port.prepare).not.toHaveBeenCalled();
  const source = c.state.sources[0];
  await c.refreshSource(source);
  expect(data.query_submit).toHaveBeenCalledTimes(1);
  c.dispose();
});
it("preserves the existing snapshot and incomplete state on a repeated page cursor", async () => {
  const { data, port } = ports();
  vi.mocked(port.read).mockResolvedValue({
    ...page,
    next: { snapshot: "2".repeat(64), offset: 128 },
  });
  const c = createSoftware(data, port);
  await c.discover();
  const source = c.state.sources[0];
  expect(c.summary.value.complete).toBe(false);
  await c.read(source, true);
  expect(source.error).toContain("未完成");
  expect(c.installed.value).toHaveLength(1);
  expect(source.after?.offset).toBe(128);
  c.dispose();
});
it("recovers a lost prepare acknowledgement by the same request and never prepares again to query", async () => {
  const { data, port } = ports();
  vi.mocked(port.prepare).mockRejectedValue(Error("lost acknowledgement"));
  const c = createSoftware(data, port);
  await c.discover();
  await c.prepare(c.installed.value[0], "uninstall");
  const request = vi.mocked(port.prepare).mock.calls[0][0].request;
  expect(port.status).toHaveBeenCalledWith({ request });
  expect(c.state.operations.has(request)).toBe(true);
  await c.query(request);
  expect(port.prepare).toHaveBeenCalledTimes(1);
  expect(port.submit).not.toHaveBeenCalled();
  c.dispose();
});
it("an installer exit or unknown effect never removes an installed record; independent absence does", async () => {
  const { data, port } = ports();
  const c = createSoftware(data, port);
  await c.discover();
  const result = original("request");
  result.execution.phase = "executionEnded";
  result.execution.attempts = 1;
  c.accept(result);
  expect(c.installed.value).toHaveLength(1);
  c.accept({ ...result, detected: { kind: "absent" } });
  expect(c.installed.value).toHaveLength(0);
  c.accept({
    ...result,
    prepared: {
      ...result.prepared,
      choice: {
        ...result.prepared.choice,
        software: {
          ...item.reference,
          cursor: { snapshot: "9".repeat(64), offset: 0 },
        },
      },
    },
    detected: { kind: "absent" },
  });
  expect(c.installed.value).toHaveLength(1);
  c.dispose();
});
it("late data from a retired user generation cannot repopulate the software projection", async () => {
  const { data, port } = ports();
  let release!: (value: SoftwarePage) => void;
  vi.mocked(port.read).mockImplementation(
    () =>
      new Promise((resolve) => {
        release = resolve;
      }),
  );
  const c = createSoftware(data, port);
  const pending = c.discover();
  await Promise.resolve();
  await Promise.resolve();
  c.dispose();
  release(page);
  await pending;
  expect(c.installed.value).toEqual([]);
  expect(c.state.sources).toEqual([]);
});
