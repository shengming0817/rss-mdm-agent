import { beforeEach, expect, it, vi } from "vitest";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { currentUser } from "../test-users";
import { nativePort, nativeDataPort, nativeTaskPort } from "./native";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
beforeEach(() => {
  currentUser.value = {
    schemaVersion: 1,
    kind: "userContext",
    user: {
      schemaVersion: 1,
      kind: "testUser",
      userId: "alice",
      displayName: "Alice",
      nameKey: "alice",
    },
    generation: "generation-alice",
    identity: {
      mode: "device",
      authorityId: "desktop-test",
      tenantId: "local-test",
      principalId: "alice",
    },
  };
  vi.mocked(invoke).mockReset();
  vi.mocked(isTauri).mockReturnValue(true);
});
it("does not expose IPC outside Tauri", () => {
  vi.mocked(isTauri).mockReturnValue(false);
  expect(nativePort()).toBeNull();
  expect(invoke).not.toHaveBeenCalled();
});
it.each(["list", "detail", "control", "controlStatus"] as const)(
  "%s preserves its command, exact envelope, response and rejection",
  async (method) => {
    const port = nativeTaskPort()!;
    const input = { instanceId: "adapter-canary" };
    const response = { response: method };
    const error = { code: "input", message: "rejected" };
    const call = () => port[method](input as never);
    vi.mocked(invoke).mockResolvedValueOnce(response);
    expect(await call()).toBe(response);
    expect(vi.mocked(invoke).mock.calls[0]).toEqual([
      `task_${method === "controlStatus" ? "control_status" : method}`,
      { input, generation: "generation-alice" },
    ]);
    vi.mocked(invoke).mockRejectedValueOnce(error);
    await expect(call()).rejects.toBe(error);
    expect(vi.mocked(invoke).mock.calls[1]).toEqual(
      vi.mocked(invoke).mock.calls[0],
    );
  },
);

it("keeps local reads and explicit queries on the same authenticated data adapter", async () => {
  const port = nativeDataPort()!;
  const input = {
    after: null,
    limit: 16,
    refreshPolicies: false,
    policyCursor: null,
  };
  vi.mocked(invoke).mockResolvedValueOnce({ items: [], next: null });
  await port.query_directory(input);
  expect(invoke).toHaveBeenLastCalledWith("query_directory", {
    input,
    generation: "generation-alice",
  });
  expect(vi.mocked(invoke).mock.calls).toHaveLength(1);
  vi.mocked(isTauri).mockReturnValue(false);
  expect(nativeDataPort()).toBeNull();
});
