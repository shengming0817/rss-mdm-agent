import { expect, it, vi } from "vitest";
import type { HostStatus } from "@rss-mdm-agent/ai-contract";
import { createHostSettings, diagnosticMessage } from "./controller";
const status = (generation: number): HostStatus => ({
  schemaVersion: 5,
  kind: "hostStatus",
  generation,
  phase: "ready",
  source: "bundled_resource",
  version: "test",
  recent: [],
});
it("clears recovered read errors while retaining the result of an export", async () => {
  const c = createHostSettings({
    read: vi
      .fn()
      .mockRejectedValueOnce(new Error("offline"))
      .mockResolvedValue(status(1)),
    restart: vi.fn(),
    export: vi.fn().mockResolvedValue(true),
  });
  await c.refresh();
  expect(c.state.readError).toContain("无法读取");
  await c.exportDiagnostics();
  await c.refresh();
  expect(c.state.readError).toBe("");
  expect(c.state.message).toBe("脱敏诊断已保存。");
});
it("late status reads cannot overwrite a completed restart and repeated clicks share the pending operation", async () => {
  let resolveRead!: (v: HostStatus) => void,
    resolveRestart!: (v: HostStatus) => void;
  const port = {
    read: vi
      .fn()
      .mockResolvedValueOnce(status(1))
      .mockImplementationOnce(() => new Promise((r) => (resolveRead = r))),
    restart: vi
      .fn()
      .mockImplementationOnce(() => new Promise((r) => (resolveRestart = r))),
    export: vi.fn(),
  };
  const c = createHostSettings(port);
  await c.refresh();
  const read = c.refresh();
  const restart = c.restart();
  await c.restart();
  expect(port.restart).toHaveBeenCalledTimes(1);
  resolveRestart(status(2));
  await restart;
  resolveRead(status(1));
  await read;
  expect(c.state.status?.generation).toBe(2);
  c.dispose();
});
it("native errors never appear verbatim and disposed controllers ignore late replies", async () => {
  let done!: (v: HostStatus) => void;
  const c = createHostSettings({
    read: () => new Promise((r) => (done = r)),
    restart: vi.fn(),
    export: vi.fn().mockRejectedValue(new Error("CANARY_SECRET")),
  });
  await c.exportDiagnostics();
  expect(c.state.message).not.toContain("CANARY");
  const pending = c.refresh();
  c.dispose();
  done(status(9));
  await pending;
  expect(c.state.status).toBeUndefined();
  expect(
    diagnosticMessage({
      ...status(1),
      phase: "failed",
      diagnostic: {
        stage: "runtime_package",
        code: "runtime_missing",
        action: "reinstall_runtime",
        atMs: 1,
      },
    }),
  ).toContain("重新安装");
});
