import { expect, it, vi } from "vitest";
import { createDiagnostics } from "./diagnostics";
import type { DiagnosticPort, DiagnosticPreview } from "./native";
const preview = (): DiagnosticPreview => ({
  snapshot: "original-snapshot",
  bytes: 24,
  content: { service: { phase: "unavailable" } },
});
it("exports exactly the preview reference and cancellation never reports success", async () => {
  const port: DiagnosticPort = {
    prepare: vi.fn(async () => preview()),
    export: vi.fn(async () => false),
    discard: vi.fn(async () => {}),
  };
  const c = createDiagnostics(port);
  await c.prepare();
  expect(port.export).not.toHaveBeenCalled();
  await c.save();
  expect(port.export).toHaveBeenCalledExactlyOnceWith("original-snapshot");
  expect(c.state.message).toBe("已取消导出。");
  expect(c.state.preview?.content).toEqual(preview().content);
  c.dispose();
});
it("discards late previews after scope or identity invalidation", async () => {
  let resolve!: (value: DiagnosticPreview) => void;
  const port: DiagnosticPort = {
    prepare: vi.fn(
      () =>
        new Promise<DiagnosticPreview>((r) => {
          resolve = r;
        }),
    ),
    export: vi.fn(),
    discard: vi.fn(async () => {}),
  };
  const c = createDiagnostics(port);
  const pending = c.prepare();
  c.clear();
  resolve(preview());
  await pending;
  expect(c.state.preview).toBeUndefined();
  expect(port.discard).toHaveBeenCalledWith("original-snapshot");
  await c.save();
  expect(port.export).not.toHaveBeenCalled();
  c.dispose();
});
it("one pending save suppresses duplicate clicks and errors remain bounded", async () => {
  let reject!: (error: Error) => void;
  const port: DiagnosticPort = {
    prepare: async () => preview(),
    export: vi.fn(
      () =>
        new Promise<boolean>((_r, r) => {
          reject = r;
        }),
    ),
    discard: async () => {},
  };
  const c = createDiagnostics(port);
  await c.prepare();
  const pending = c.save();
  await c.save();
  expect(port.export).toHaveBeenCalledTimes(1);
  reject(new Error("CANARY_SECRET"));
  await pending;
  expect(c.state.message).toContain("未导出");
  expect(c.state.message).not.toContain("CANARY");
  c.dispose();
});
