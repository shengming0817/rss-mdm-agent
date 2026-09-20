import { beforeEach, expect, it, vi } from "vitest";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { currentUser } from "../test-users";
import { nativePort } from "./native";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
beforeEach(() => {
  currentUser.value = {
    schemaVersion: 5,
    kind: "userContext",
    user: {
      schemaVersion: 5,
      kind: "testUser",
      userId: "alice",
      displayName: "Alice",
      nameKey: "alice",
    },
    generation: "generation-alice",
  };
  vi.mocked(invoke).mockReset();
  vi.mocked(isTauri).mockReturnValue(true);
});
it("does not expose IPC outside Tauri", () => {
  vi.mocked(isTauri).mockReturnValue(false);
  expect(nativePort()).toBeNull();
  expect(invoke).not.toHaveBeenCalled();
});
it.each(["snapshot", "preview", "submit", "respond"] as const)(
  "%s preserves its command, exact envelope, response and rejection",
  async (method) => {
    const port = nativePort()!;
    const input = { instanceId: "adapter-canary" };
    const response = { response: method };
    const error = { code: "input", message: "rejected" };
    const call = () => port[method](input as never);
    vi.mocked(invoke).mockResolvedValueOnce(response);
    expect(await call()).toBe(response);
    expect(vi.mocked(invoke).mock.calls[0]).toEqual([
      `self_service_${method}`,
      { input, generation: "generation-alice" },
    ]);
    vi.mocked(invoke).mockRejectedValueOnce(error);
    await expect(call()).rejects.toBe(error);
    expect(vi.mocked(invoke).mock.calls[1]).toEqual(
      vi.mocked(invoke).mock.calls[0],
    );
  },
);
